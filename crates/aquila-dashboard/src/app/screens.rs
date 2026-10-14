//! The ten screens of the dashboard (`docs/spec/dashboard.md` §6).
//!
//! Every screen is a thin renderer over the shared [`Ctx`] signals that the
//! poller fills from the core's private control plane. Screens send intent
//! through the [`CoreApi`](crate::api::CoreApi) seam; they never derive domain
//! state and never keep sensing history.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use super::ui::{select_value, value_of, EmptyState, StatusBadge};
use super::Ctx;
use crate::api::CoreApi;
use crate::viewmodel::{
    format_count, format_metric, health_view, node_state_counts, overview_view, pair_label,
    structure_view, validate_new_room, FieldError,
};

/// `#/overview` — whole-home occupancy and per-room vitals cards.
#[component]
pub fn Overview() -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    view! {
        <h1>"Overview"</h1>
        <section class="card" aria-labelledby="home-heading">
            <h2 id="home-heading">"Whole home"</h2>
            {move || {
                let view = overview_view(&ctx.snapshot.get().unwrap_or_default());
                if !view.has_data {
                    view! {
                        <EmptyState
                            title="No rooms reporting yet"
                            hint="Commission nodes on the Nodes screen, then run calibration."
                        />
                    }
                    .into_any()
                } else {
                    let state = if view.occupied { "online" } else { "unknown" };
                    let label = if view.occupied { "Occupied" } else { "Unoccupied" };
                    view! {
                        <p>
                            <StatusBadge state=state label=label />
                            <span class="badge">{format!("{} people", view.people)}</span>
                        </p>
                    }
                    .into_any()
                }
            }}
        </section>

        <div class="grid" aria-label="Rooms">
            {move || {
                let view = overview_view(&ctx.snapshot.get().unwrap_or_default());
                if view.rooms.is_empty() {
                    return ().into_any();
                }
                view.rooms
                    .into_iter()
                    .map(|room| {
                        let href = format!("#/rooms/{}", room.id);
                        let state = match room.presence {
                            Some(true) => "online",
                            Some(false) => "offline",
                            None => "unknown",
                        };
                        view! {
                            <a class="card" href=href>
                                <h3>{room.name.clone()}</h3>
                                <StatusBadge state=state label=room.presence_label() />
                                <p>{format!(
                                    "People: {}",
                                    format_count(room.count),
                                )}</p>
                                <p>{format!(
                                    "Breathing: {}",
                                    format_metric(room.breathing, "brpm"),
                                )}</p>
                                <p>{format!(
                                    "Heart: {}",
                                    format_metric(room.heart, "bpm"),
                                )}</p>
                            </a>
                        }
                    })
                    .collect_view()
                    .into_any()
            }}
        </div>
    }
}

/// `#/rooms/{id}` — the live view for one room.
#[component]
pub fn RoomScreen(id: String) -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    let room_id = id.clone();
    view! {
        <h1>"Room live view"</h1>
        <section class="card">
            <button
                on:click=move |_| ctx.live.update(|live| *live = !*live)
            >
                {move || if ctx.live.get() { "Pause live" } else { "Resume live" }}
            </button>
            <p class="muted" aria-live="off">
                {move || if ctx.live.get() { "Live updates running." } else { "Live updates paused; showing the last snapshot." }}
            </p>
        </section>
        {move || {
            let snapshot = ctx.snapshot.get().unwrap_or_default();
            match snapshot.zones.get(&room_id) {
                None => view! {
                    <EmptyState
                        title="Room not reporting"
                        hint="This room has no live readings yet. Check its nodes on the Nodes screen."
                    />
                }
                .into_any(),
                Some(zone) => {
                    let name = if zone.name.is_empty() { room_id.clone() } else { zone.name.clone() };
                    let state = match zone.presence {
                        Some(true) => "online",
                        Some(false) => "offline",
                        None => "unknown",
                    };
                    let label = match zone.presence {
                        Some(true) => "Present",
                        Some(false) => "Absent",
                        None => "Unknown",
                    };
                    view! {
                        <section class="card" aria-labelledby="room-heading">
                            <h2 id="room-heading">{name}</h2>
                            <StatusBadge state=state label=label />
                            <div class="vitals">
                                <p>{format!("People: {}", format_count(zone.person_count))}</p>
                                <p>{format!("Breathing: {}", format_metric(zone.breathing_rate, "brpm"))}</p>
                                <p>{format!("Heart: {}", format_metric(zone.heart_rate, "bpm"))}</p>
                            </div>
                            <p class="muted">
                                "Coarse pose and zone view arrive with the sensing stream (a future ticket); \
                                 v1 polls presence, count and vitals only."
                            </p>
                        </section>
                    }
                    .into_any()
                }
            }
        }}
    }
}

/// `#/structure` — author floors and rooms.
#[component]
pub fn Structure() -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    let floor = RwSignal::new(String::new());
    let name = RwSignal::new(String::new());
    let label = RwSignal::new(String::new());
    let errors = RwSignal::new(Vec::<FieldError>::new());

    let submit = {
        let ctx = ctx.clone();
        move |_| {
            match validate_new_room(&floor.get(), &name.get(), &label.get()) {
                Err(list) => errors.set(list),
                Ok(room) => {
                    errors.set(Vec::new());
                    let client = ctx.client.clone();
                    let floor_id = floor.get();
                    spawn_local(async move {
                        if let Err(error) = client.create_room(&floor_id, &room).await {
                            ctx.error.set(Some(error.operator_message()));
                        }
                    });
                }
            }
        }
    };

    view! {
        <h1>"Structure"</h1>
        <section class="card" aria-labelledby="tree-heading">
            <h2 id="tree-heading">{move || structure_view(&ctx.schema.get().unwrap_or_default()).home_name}</h2>
            {move || {
                let view = structure_view(&ctx.schema.get().unwrap_or_default());
                if view.floors.is_empty() {
                    view! {
                        <EmptyState
                            title="No floors yet"
                            hint="Create a floor, then add rooms to it."
                        />
                    }
                    .into_any()
                } else {
                    view! {
                        <ul>
                            {view.floors.into_iter().map(|floor| {
                                let floor_name = floor.name.clone();
                                let rooms = if floor.rooms.is_empty() {
                                    view! { <p class="muted">"No rooms yet."</p> }.into_any()
                                } else {
                                    view! {
                                        <ul>
                                            {floor.rooms.into_iter().map(|room| {
                                                let href = format!("#/rooms/{}", room.id);
                                                view! {
                                                    <li>
                                                        <a href=href>{room.name.clone()}</a>
                                                        <span class="badge">{room.kind.clone()}</span>
                                                    </li>
                                                }
                                            }).collect_view()}
                                        </ul>
                                    }
                                    .into_any()
                                };
                                view! {
                                    <li>
                                        <h3>{floor_name}</h3>
                                        {rooms}
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                    }
                    .into_any()
                }
            }}
        </section>

        <section class="card" aria-labelledby="new-room-heading">
            <h2 id="new-room-heading">"Add a room"</h2>
            <div class="inline">
                <div>
                    <label for="room-floor">"Floor"</label>
                    <select
                        id="room-floor"
                        on:change=move |event: web_sys::Event| floor.set(select_value(&event))
                    >
                        <option value="">"Select a floor…"</option>
                        {move || {
                            ctx.schema
                                .get()
                                .unwrap_or_default()
                                .floors
                                .into_iter()
                                .map(|floor| view! { <option value=floor.id.clone()>{floor.name.clone()}</option> })
                                .collect_view()
                        }}
                    </select>
                </div>
                <div>
                    <label for="room-name">"Room name"</label>
                    <input
                        id="room-name"
                        type="text"
                        prop:value=move || name.get()
                        aria-describedby="room-name-error"
                        on:input=move |event: web_sys::Event| name.set(value_of(&event))
                    />
                </div>
                <div>
                    <label for="room-label">"Kind"</label>
                    <input
                        id="room-label"
                        type="text"
                        placeholder="living, kitchen, study…"
                        prop:value=move || label.get()
                        on:input=move |event: web_sys::Event| label.set(value_of(&event))
                    />
                </div>
                <button class="primary" on:click=submit>"Add room"</button>
            </div>
            <div id="room-name-error">
                {move || {
                    errors
                        .get()
                        .into_iter()
                        .map(|error| view! { <p class="error-text">{error.message}</p> })
                        .collect_view()
                }}
            </div>
        </section>
    }
}

/// `#/nodes` — commission and inspect nodes.
#[component]
pub fn Nodes() -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    let name = RwSignal::new(String::new());
    let mac = RwSignal::new(String::new());
    let room = RwSignal::new(String::new());

    let register = {
        let ctx = ctx.clone();
        move |_| {
            let body = crate::contract::NewNode {
                name: name.get(),
                mac: mac.get(),
                room_id: room.get(),
            };
            let client = ctx.client.clone();
            spawn_local(async move {
                if let Err(error) = client.register_node(&body).await {
                    ctx.error.set(Some(error.operator_message()));
                }
            });
        }
    };

    view! {
        <h1>"Nodes"</h1>
        <section class="card" aria-labelledby="node-list-heading">
            <h2 id="node-list-heading">"Registered nodes"</h2>
            {move || {
                let nodes = ctx.nodes.get();
                if nodes.is_empty() {
                    view! {
                        <EmptyState
                            title="No nodes registered"
                            hint="Flash a node, then commission it here with its MAC address."
                        />
                    }
                    .into_any()
                } else {
                    let counts = node_state_counts(&nodes)
                        .into_iter()
                        .map(|(state, count)| format!("{count} {state}"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    view! {
                        <p class="muted">{counts}</p>
                        <table>
                            <caption class="sr-only">"Registered esp32 nodes"</caption>
                            <thead>
                                <tr>
                                    <th scope="col">"Name"</th>
                                    <th scope="col">"State"</th>
                                    <th scope="col">"MAC"</th>
                                    <th scope="col">"Room"</th>
                                    <th scope="col">"Firmware"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {nodes.into_iter().map(|node| {
                                    view! {
                                        <tr>
                                            <td>{node.name.clone()}</td>
                                            <td><StatusBadge state=node.state.clone() label=node.state.clone() /></td>
                                            <td>{node.mac.clone()}</td>
                                            <td>{node.room_id.clone()}</td>
                                            <td>{node.firmware.installed_version.clone()}</td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    }
                    .into_any()
                }
            }}
        </section>

        <section class="card" aria-labelledby="register-heading">
            <h2 id="register-heading">"Commission a node"</h2>
            <div class="inline">
                <div>
                    <label for="node-name">"Node name"</label>
                    <input id="node-name" type="text" prop:value=move || name.get()
                        on:input=move |event: web_sys::Event| name.set(value_of(&event)) />
                </div>
                <div>
                    <label for="node-mac">"MAC address"</label>
                    <input id="node-mac" type="text" placeholder="aa:bb:cc:dd:ee:ff"
                        prop:value=move || mac.get()
                        on:input=move |event: web_sys::Event| mac.set(value_of(&event)) />
                </div>
                <div>
                    <label for="node-room">"Room"</label>
                    <select id="node-room" on:change=move |event: web_sys::Event| room.set(select_value(&event))>
                        <option value="">"Select a room…"</option>
                        {move || {
                            ctx.schema.get().unwrap_or_default().rooms.into_iter()
                                .map(|room| view! { <option value=room.id.clone()>{room.name.clone()}</option> })
                                .collect_view()
                        }}
                    </select>
                </div>
                <button class="primary" on:click=register>"Commission"</button>
            </div>
        </section>
    }
}

/// `#/pairs` — declare and inspect node pairs.
#[component]
pub fn Pairs() -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    let node_a = RwSignal::new(String::new());
    let node_b = RwSignal::new(String::new());

    let create = {
        let ctx = ctx.clone();
        move |_| {
            let body = crate::contract::NewPair {
                node_a: node_a.get(),
                node_b: node_b.get(),
            };
            let client = ctx.client.clone();
            spawn_local(async move {
                if let Err(error) = client.create_pair(&body).await {
                    ctx.error.set(Some(error.operator_message()));
                }
            });
        }
    };
    let derive = {
        let ctx = ctx.clone();
        move |_| {
            let client = ctx.client.clone();
            spawn_local(async move {
                match client.derive_pairs().await {
                    Ok(pairs) => ctx.pairs.set(pairs),
                    Err(error) => ctx.error.set(Some(error.operator_message())),
                }
            });
        }
    };

    view! {
        <h1>"Node pairs"</h1>
        <section class="card" aria-labelledby="pair-list-heading">
            <h2 id="pair-list-heading">"Declared pairs"</h2>
            <button on:click=derive>"Suggest pairs from calibration"</button>
            {move || {
                let pairs = ctx.pairs.get();
                if pairs.is_empty() {
                    view! {
                        <EmptyState
                            title="No node pairs"
                            hint="Declare a pair, or derive suggestions from calibration data."
                        />
                    }
                    .into_any()
                } else {
                    view! {
                        <table>
                            <caption class="sr-only">"Node pairs"</caption>
                            <thead>
                                <tr>
                                    <th scope="col">"Pair"</th>
                                    <th scope="col">"Enabled"</th>
                                    <th scope="col">"Quality"</th>
                                    <th scope="col">"Source"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {pairs.into_iter().map(|pair| {
                                    let label = pair_label(&pair);
                                    view! {
                                        <tr>
                                            <td>{label}</td>
                                            <td>{if pair.enabled { "Yes" } else { "No" }}</td>
                                            <td>{format_metric(pair.quality, "")}</td>
                                            <td>{pair.source.clone()}</td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    }
                    .into_any()
                }
            }}
        </section>

        <section class="card" aria-labelledby="new-pair-heading">
            <h2 id="new-pair-heading">"Declare a pair"</h2>
            <div class="inline">
                <div>
                    <label for="pair-a">"Node A (transmits)"</label>
                    <select id="pair-a" on:change=move |event: web_sys::Event| node_a.set(select_value(&event))>
                        <option value="">"Select a node…"</option>
                        {move || {
                            ctx.nodes.get().into_iter()
                                .map(|node| view! { <option value=node.id.clone()>{node.name.clone()}</option> })
                                .collect_view()
                        }}
                    </select>
                </div>
                <div>
                    <label for="pair-b">"Node B (receives)"</label>
                    <select id="pair-b" on:change=move |event: web_sys::Event| node_b.set(select_value(&event))>
                        <option value="">"Select a node…"</option>
                        {move || {
                            ctx.nodes.get().into_iter()
                                .map(|node| view! { <option value=node.id.clone()>{node.name.clone()}</option> })
                                .collect_view()
                        }}
                    </select>
                </div>
                <button class="primary" on:click=create>"Declare pair"</button>
            </div>
        </section>
    }
}

/// `#/calibrate` — the global calibration wizard.
#[component]
pub fn Calibrate() -> impl IntoView {
    let ctx = expect_context::<Ctx>();

    view! {
        <h1>"Calibration"</h1>
        <section class="card" aria-labelledby="calibration-heading">
            <h2 id="calibration-heading">"Global whole-home calibration"</h2>
            {move || match ctx.calibration.get() {
                None => {
                    let ctx_start = ctx.clone();
                    view! {
                        <div>
                            <EmptyState
                                title="No calibration session"
                                hint="Start the wizard once all nodes are online and time-sync is healthy."
                            />
                            <button class="primary" on:click=move |_| {
                                let client = ctx_start.client.clone();
                                spawn_local(async move {
                                    match client.start_calibration().await {
                                        Ok(session) => ctx_start.calibration.set(Some(session)),
                                        Err(error) => ctx_start.error.set(Some(error.operator_message())),
                                    }
                                });
                            }>"Start calibration"</button>
                        </div>
                    }
                    .into_any()
                }
                Some(session) => {
                    let session_id = session.id.clone();
                    let status = session.status.clone();
                    let steps = session.steps.clone();
                    let complete_id = session_id.clone();
                    let skip_id = session_id.clone();
                    let commit_id = session_id.clone();
                    let abort_id = session_id.clone();
                    let ctx_complete = ctx.clone();
                    let ctx_skip = ctx.clone();
                    let ctx_commit = ctx.clone();
                    let ctx_abort = ctx.clone();
                    view! {
                        <div>
                            <p>
                                <span class="badge">{format!("Status: {status}")}</span>
                                <span class="badge">{format!("Scope: {}", session.scope)}</span>
                            </p>
                            <ol>
                                {steps.into_iter().map(|step| {
                                    let awaiting = if step.awaiting == "user" { "needs you" } else { "automatic" };
                                    let progress = step.progress.map(|value| format!("{:.0}%", value * 100.0))
                                        .unwrap_or_else(|| "—".to_string());
                                    view! {
                                        <li>
                                            <p>{step.instruction.clone()}</p>
                                            <p class="muted">
                                                {format!("{} · {} · {}", step.kind, awaiting, progress)}
                                            </p>
                                        </li>
                                    }
                                }).collect_view()}
                            </ol>
                            <div class="inline">
                                <button on:click=move |_| {
                                    let client = ctx_complete.client.clone();
                                    let sid = complete_id.clone();
                                    let step = next_step_id(&ctx_complete);
                                    spawn_local(async move {
                                        if let Some(step) = step {
                                            if let Err(error) = client.complete_step(&sid, &step).await {
                                                ctx_complete.error.set(Some(error.operator_message()));
                                            }
                                        }
                                    });
                                }>"Complete step"</button>
                                <button on:click=move |_| {
                                    let client = ctx_skip.client.clone();
                                    let sid = skip_id.clone();
                                    let step = next_step_id(&ctx_skip);
                                    spawn_local(async move {
                                        if let Some(step) = step {
                                            if let Err(error) = client.skip_step(&sid, &step).await {
                                                ctx_skip.error.set(Some(error.operator_message()));
                                            }
                                        }
                                    });
                                }>"Skip step"</button>
                                <button class="primary" on:click=move |_| {
                                    let client = ctx_commit.client.clone();
                                    let sid = commit_id.clone();
                                    spawn_local(async move {
                                        if let Err(error) = client.commit_calibration(&sid).await {
                                            ctx_commit.error.set(Some(error.operator_message()));
                                        }
                                    });
                                }>"Commit"</button>
                                <button on:click=move |_| {
                                    let client = ctx_abort.client.clone();
                                    let sid = abort_id.clone();
                                    spawn_local(async move {
                                        match client.abort_calibration(&sid).await {
                                            Ok(_) => ctx_abort.calibration.set(None),
                                            Err(error) => ctx_abort.error.set(Some(error.operator_message())),
                                        }
                                    });
                                }>"Abort"</button>
                            </div>
                        </div>
                    }
                    .into_any()
                }
            }}
        </section>
    }
}

/// The current step id of the active session, if any.
fn next_step_id(ctx: &Ctx) -> Option<String> {
    ctx.calibration
        .get_untracked()
        .and_then(|session| session.current_step_id)
}

/// `#/health` — subsystem health and pipeline load.
#[component]
pub fn HealthScreen() -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    view! {
        <h1>"System health"</h1>
        <section class="card" aria-labelledby="health-heading">
            <h2 id="health-heading">"Subsystems"</h2>
            {move || {
                let health = ctx.health.get().unwrap_or_default();
                let view = health_view(&health);
                let subsystems = health.subsystems.clone();
                view! {
                    <p><StatusBadge state=view.status.clone() label=view.status.clone() /></p>
                    <p>{view.summary}</p>
                    <table>
                        <caption class="sr-only">"Subsystem health"</caption>
                        <thead>
                            <tr><th scope="col">"Subsystem"</th><th scope="col">"Status"</th><th scope="col">"Detail"</th></tr>
                        </thead>
                        <tbody>
                            {subsystems.into_iter().map(|subsystem| {
                                let detail = subsystem.detail.clone().unwrap_or_default();
                                view! {
                                    <tr>
                                        <td>{subsystem.name.clone()}</td>
                                        <td><StatusBadge state=subsystem.status.clone() label=subsystem.status.clone() /></td>
                                        <td>{detail}</td>
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    </table>
                }
            }}
        </section>

        <section class="card" aria-labelledby="bars-heading">
            <h2 id="bars-heading">"Accuracy bars"</h2>
            {move || {
                let metrics = ctx.metrics.get().unwrap_or_default();
                if metrics.bars.is_empty() {
                    return view! { <EmptyState title="No bars yet" hint="The measurement ledger is empty." /> }.into_any();
                }
                view! {
                    <table>
                        <caption class="sr-only">"Accuracy bars"</caption>
                        <thead>
                            <tr><th scope="col">"Bar"</th><th scope="col">"Target"</th><th scope="col">"Observed"</th><th scope="col">"Result"</th></tr>
                        </thead>
                        <tbody>
                            {metrics.bars.into_iter().map(|(name, bar)| {
                                let result = if bar.pass { "Pass" } else { "Fail" };
                                let state = if bar.pass { "ok" } else { "down" };
                                view! {
                                    <tr>
                                        <td>{name}</td>
                                        <td>{format!("{:.2}", bar.target)}</td>
                                        <td>{format!("{:.2}", bar.observed)}</td>
                                        <td><StatusBadge state=state label=result /></td>
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    </table>
                }
                .into_any()
            }}
        </section>
    }
}

/// `#/measurements` — the commit-keyed ledger snapshot (read-only).
#[component]
pub fn Measurements() -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    let kind = RwSignal::new(String::new());
    view! {
        <h1>"Measurement ledger"</h1>
        <section class="card" aria-labelledby="ledger-heading">
            <h2 id="ledger-heading">"Current commit"</h2>
            <p class="muted" aria-live="off">
                {move || match ctx.metrics.get().and_then(|metrics| metrics.commit) {
                    Some(commit) => format!("Commit {commit}"),
                    None => "No ledger commit yet; the snapshot is empty.".to_string(),
                }}
            </p>
            <div class="inline">
                <label for="metric-kind">"Filter by kind"</label>
                <select id="metric-kind" on:change=move |event: web_sys::Event| kind.set(select_value(&event))>
                    <option value="">"All kinds"</option>
                    <option value="deterministic">"Deterministic"</option>
                    <option value="statistical">"Statistical"</option>
                    <option value="gated">"Gated"</option>
                </select>
            </div>
            {move || {
                let metrics = ctx.metrics.get().unwrap_or_default();
                let filter = kind.get();
                let rows: Vec<_> = metrics
                    .metrics
                    .into_iter()
                    .filter(|metric| filter.is_empty() || metric.kind == filter)
                    .collect();
                if rows.is_empty() {
                    return view! {
                        <EmptyState
                            title="No metrics"
                            hint="The ledger snapshot carries no metrics for this filter."
                        />
                    }
                    .into_any();
                }
                view! {
                    <table>
                        <caption class="sr-only">"Ledger metrics"</caption>
                        <thead>
                            <tr>
                                <th scope="col">"Metric"</th>
                                <th scope="col">"Kind"</th>
                                <th scope="col">"Value"</th>
                                <th scope="col">"Unit"</th>
                                <th scope="col">"N"</th>
                                <th scope="col">"Gate"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {rows.into_iter().map(|metric| {
                                let gate = metric.gate.as_ref().map(|gate| {
                                    if gate.pass { "Pass" } else { "Fail" }
                                }).unwrap_or("—");
                                view! {
                                    <tr>
                                        <td>{metric.name.clone()}</td>
                                        <td>{metric.kind.clone()}</td>
                                        <td>{format!("{:.4}", metric.value)}</td>
                                        <td>{metric.unit.clone()}</td>
                                        <td>{metric.n.to_string()}</td>
                                        <td>{gate}</td>
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    </table>
                }
                .into_any()
            }}
        </section>
    }
}

/// `#/settings` — units, locale, theme, privacy and about.
#[component]
pub fn Settings() -> impl IntoView {
    let ctx = expect_context::<Ctx>();
    view! {
        <h1>"Settings"</h1>
        <section class="card" aria-labelledby="home-settings-heading">
            <h2 id="home-settings-heading">"Home"</h2>
            {move || {
                let schema = ctx.schema.get().unwrap_or_default();
                view! {
                    <dl>
                        <dt>"Name"</dt><dd>{schema.home.name.clone()}</dd>
                        <dt>"Time zone"</dt><dd>{schema.home.time_zone.clone()}</dd>
                        <dt>"Instance"</dt><dd>{schema.instance_id.clone()}</dd>
                        <dt>"Contract"</dt><dd>{schema.contract.clone()}</dd>
                    </dl>
                }
            }}
        </section>
        <section class="card" aria-labelledby="display-heading">
            <h2 id="display-heading">"Display"</h2>
            <p>"Units, locale and theme follow the browser and Home Assistant; the dashboard ships a self-contained theme driven by "<code>"prefers-color-scheme"</code>"."</p>
        </section>
        <section class="card" aria-labelledby="privacy-heading">
            <h2 id="privacy-heading">"Privacy"</h2>
            <p>"Recording toggles and retention are set in the core add-on options and enforced there; the dashboard only reflects them. No telemetry is ever sent."</p>
        </section>
        <section class="card" aria-labelledby="about-heading">
            <h2 id="about-heading">"About"</h2>
            <p>{format!("{} — management dashboard.", crate::NAME)}</p>
            <p>"GPLv3. Non-commercial use. The sensing core keeps every claim honest and reproducible in the commit-keyed ledger."</p>
        </section>
    }
}

/// The route-not-found screen.
#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <h1>"Not found"</h1>
        <EmptyState
            title="No such screen"
            hint="Use the navigation to reach Overview, Structure, Nodes, Node pairs, Calibration, System health, Measurements or Settings."
        />
    }
}
