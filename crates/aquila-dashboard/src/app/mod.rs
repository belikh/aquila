//! The Leptos CSR application: shell, hash routing, polling and the screens
//! (`docs/spec/dashboard.md` §4, §5, §6).
//!
//! Compiled only for `wasm32` (see `lib.rs`). The app polls the canonical
//! `/api/v1` HTTP routes on an interval with backoff (`core-api.md` §10.1); it
//! opens no WebSocket, because v1's stream is a typed `501`.

pub mod screens;
pub mod ui;

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::contract::{
    CalibrationSession, Health, Home, MetricsSnapshot, Node, NodePair, Schema, Snapshot,
};
use crate::net::HttpCoreApi;
use crate::router::{nav_entries, Route};
use crate::viewmodel::{live_summary, overview_view};
use crate::CoreApi;

use self::screens::{
    Calibrate, HealthScreen, Measurements, Nodes, NotFound, Overview, Pairs, RoomScreen, Settings,
    Structure,
};
use self::ui::StatusBadge;

/// Mount the SPA into the document body. Trunk's binary entry calls this.
pub fn mount() {
    leptos::mount::mount_to_body(App);
}

/// The shared application state: one client and the signals every screen reads.
///
/// Signals are `Copy`, and the client is cheaply cloneable, so the whole
/// context is `Clone`. It is provided once and read with `expect_context`.
#[derive(Clone)]
pub struct Ctx {
    /// The current route.
    pub route: RwSignal<Route>,
    /// The HTTP client of the core's private control plane.
    pub client: HttpCoreApi,
    /// `GET /schema` — home, floors and rooms.
    pub schema: RwSignal<Option<Schema>>,
    /// `GET /state` — the live snapshot.
    pub snapshot: RwSignal<Option<Snapshot>>,
    /// `GET /health`.
    pub health: RwSignal<Option<Health>>,
    /// `GET /nodes`.
    pub nodes: RwSignal<Vec<Node>>,
    /// `GET /node-pairs`.
    pub pairs: RwSignal<Vec<NodePair>>,
    /// `GET /metrics/snapshot`.
    pub metrics: RwSignal<Option<MetricsSnapshot>>,
    /// `GET /calibration/current`.
    pub calibration: RwSignal<Option<CalibrationSession>>,
    /// `GET /home`.
    pub home: RwSignal<Option<Home>>,
    /// Whether the last poll failed (the snapshot is then stale).
    pub stale: RwSignal<bool>,
    /// The last operator-facing error, surfaced with `role="alert"`.
    pub error: RwSignal<Option<String>>,
    /// The throttled `aria-live` summary.
    pub notice: RwSignal<String>,
    /// Whether the live view is updating (Room screen pause/resume).
    pub live: RwSignal<bool>,
    /// The wall-clock time of the last live announcement, for throttling.
    pub last_notice_ms: RwSignal<f64>,
}

impl Ctx {
    fn new(route: RwSignal<Route>) -> Self {
        Self {
            route,
            client: HttpCoreApi::from_document(),
            schema: RwSignal::new(None),
            snapshot: RwSignal::new(None),
            health: RwSignal::new(None),
            nodes: RwSignal::new(Vec::new()),
            pairs: RwSignal::new(Vec::new()),
            metrics: RwSignal::new(None),
            calibration: RwSignal::new(None),
            home: RwSignal::new(None),
            stale: RwSignal::new(false),
            error: RwSignal::new(None),
            notice: RwSignal::new(String::new()),
            live: RwSignal::new(true),
            last_notice_ms: RwSignal::new(0.0),
        }
    }
}

/// The application root.
#[component]
pub fn App() -> impl IntoView {
    let route = RwSignal::new(current_route());
    install_hash_listener(route);
    let ctx = Ctx::new(route);
    provide_context(ctx.clone());
    spawn_poller(ctx);
    view! { <Shell/> }
}

/// The ingress-frame shell: landmarks, nav, live region and the route view
/// (`dashboard.md` §7).
#[component]
fn Shell() -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    view! {
        <a class="skip-link" href="#main">"Skip to main content"</a>
        <header class="app-header">
            <span class="brand">{crate::NAME}</span>
            <SystemStatus/>
            <nav class="app-nav" aria-label="Primary">
                <ul>
                    {nav_entries()
                        .into_iter()
                        .filter(|entry| *entry != Route::NotFound)
                        .map(|entry| {
                            let label = entry.title();
                            let href = entry.to_hash();
                            let current = entry.clone();
                            view! {
                                <li>
                                    <a
                                        href=href
                                        aria-current=move || {
                                            if ctx.route.get().is_entry(&current) { Some("page") } else { None }
                                        }
                                    >
                                        {label}
                                    </a>
                                </li>
                            }
                        })
                        .collect_view()}
                </ul>
            </nav>
        </header>
        <main id="main" tabindex="-1">
            <LiveRegion/>
            <ErrorBanner/>
            <RouteView/>
        </main>
    }
}

/// The system status pill: core reachability and health.
#[component]
fn SystemStatus() -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    view! {
        <span>
            {move || {
                if ctx.stale.get() {
                    view! { <StatusBadge state="down" label="Core unreachable" /> }.into_any()
                } else {
                    let health = ctx.health.get().unwrap_or_default();
                    let status = if health.status.is_empty() {
                        "unknown".to_string()
                    } else {
                        health.status.clone()
                    };
                    let label = match status.as_str() {
                        "ok" => "Core healthy",
                        "degraded" => "Core degraded",
                        "down" => "Core down",
                        _ => "Core status unknown",
                    };
                    view! { <StatusBadge state=status label=label /> }.into_any()
                }
            }}
        </span>
    }
}

/// The single throttled `aria-live` region (`dashboard.md` §7).
#[component]
fn LiveRegion() -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    view! {
        <div class="sr-only" role="status" aria-live="polite">
            {move || ctx.notice.get()}
        </div>
    }
}

/// The last error, announced assertively (`dashboard.md` §6 failure states).
#[component]
fn ErrorBanner() -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    view! {
        {move || {
            ctx.error
                .get()
                .map(|message| view! { <p class="error-text" role="alert">{message}</p> })
        }}
    }
}

/// Render the component for the current route.
#[component]
fn RouteView() -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    move || match ctx.route.get() {
        Route::Overview => view! { <Overview/> }.into_any(),
        Route::Room { id } => view! { <RoomScreen id=id/> }.into_any(),
        Route::Structure => view! { <Structure/> }.into_any(),
        Route::Nodes => view! { <Nodes/> }.into_any(),
        Route::Pairs => view! { <Pairs/> }.into_any(),
        Route::Calibrate => view! { <Calibrate/> }.into_any(),
        Route::Health => view! { <HealthScreen/> }.into_any(),
        Route::Measurements => view! { <Measurements/> }.into_any(),
        Route::Settings => view! { <Settings/> }.into_any(),
        Route::NotFound => view! { <NotFound/> }.into_any(),
    }
}

/// The route for the current fragment.
fn current_route() -> Route {
    Route::parse(&current_hash())
}

/// The current `location.hash`, or an empty string.
fn current_hash() -> String {
    web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .unwrap_or_default()
}

/// Install the `hashchange` listener that drives in-app navigation
/// (`dashboard.md` §4).
fn install_hash_listener(route: RwSignal<Route>) {
    let closure = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
        route.set(current_route());
        focus_main();
    });
    if let Some(window) = web_sys::window() {
        let _ =
            window.add_event_listener_with_callback("hashchange", closure.as_ref().unchecked_ref());
    }
    // The listener lives for the whole page session; forgetting the closure is
    // deliberate and leaks nothing that matters for a single-page app.
    closure.forget();
}

/// Move focus to `<main>` after a route change so the next Tab starts in the
/// new screen (`dashboard.md` §7).
fn focus_main() {
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("main"))
    {
        if let Ok(html) = element.dyn_into::<web_sys::HtmlElement>() {
            let _ = html.focus();
        }
    }
}

/// Start the poll loop (`core-api.md` §10.1).
///
/// Capabilities are read once and the major version is checked before the loop
/// starts (`core-api.md` §4.2). Each iteration polls the routes the screens
/// need, marks staleness on failure, and backs off with jitter.
fn spawn_poller(ctx: Ctx) {
    spawn_local(async move {
        match ctx.client.capabilities().await {
            Ok(capabilities) if capabilities.api.major != crate::API_MAJOR => {
                ctx.error.set(Some(format!(
                    "core API major {} is unsupported by this dashboard (v{})",
                    capabilities.api.major,
                    crate::API_MAJOR
                )));
            }
            Ok(_) => {}
            Err(error) => ctx.error.set(Some(error.operator_message())),
        }

        let mut backoff: u32 = 1000;
        loop {
            let ok = poll_once(&ctx).await;
            if ok {
                backoff = 1000;
            } else {
                ctx.stale.set(true);
                backoff = (backoff * 2).min(30_000);
            }
            let jitter = (js_sys::Math::random() * 0.2 * f64::from(backoff)) as u32;
            gloo_timers::future::TimeoutFuture::new(backoff + jitter).await;
        }
    });
}

/// One poll of the routes the screens render. Returns whether the core was
/// reachable.
async fn poll_once(ctx: &Ctx) -> bool {
    let mut ok = true;

    match ctx.client.health().await {
        Ok(health) => ctx.health.set(Some(health)),
        Err(error) => {
            ctx.error.set(Some(error.operator_message()));
            ok = false;
        }
    }

    match ctx.client.schema().await {
        Ok(schema) => ctx.schema.set(Some(schema)),
        Err(error) => {
            ctx.error.set(Some(error.operator_message()));
            ok = false;
        }
    }

    if ctx.live.get_untracked() {
        match ctx.client.snapshot().await {
            Ok(snapshot) => {
                emit_live(ctx, &snapshot);
                ctx.snapshot.set(Some(snapshot));
            }
            Err(error) => {
                ctx.error.set(Some(error.operator_message()));
                ok = false;
            }
        }
    }

    match ctx.client.nodes().await {
        Ok(nodes) => ctx.nodes.set(nodes),
        Err(_) => ok = false,
    }

    // Best-effort reads. The node-pair, calibration and home topology routes
    // are the dashboard's contract (`core-api.md` §6) but the server side is
    // not wired in this build, so a `404` here must not mark the whole poll
    // stale.
    if let Ok(pairs) = ctx.client.node_pairs().await {
        ctx.pairs.set(pairs);
    }
    if let Ok(metrics) = ctx.client.metrics_snapshot().await {
        ctx.metrics.set(Some(metrics));
    }
    if let Ok(calibration) = ctx.client.current_calibration().await {
        ctx.calibration.set(calibration);
    }
    if let Ok(home) = ctx.client.home().await {
        ctx.home.set(Some(home));
    }

    if ok {
        ctx.stale.set(false);
    }
    ok
}

/// Announce a summarised live update, throttled to at most once every five
/// seconds and only when the summary changes (`dashboard.md` §7).
fn emit_live(ctx: &Ctx, snapshot: &Snapshot) {
    let summary = live_summary(&overview_view(snapshot));
    let now = js_sys::Date::now();
    if summary != ctx.notice.get_untracked() && now - ctx.last_notice_ms.get_untracked() >= 5_000.0
    {
        ctx.notice.set(summary);
        ctx.last_notice_ms.set(now);
    }
}
