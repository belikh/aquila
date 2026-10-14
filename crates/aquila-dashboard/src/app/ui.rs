//! Small presentational components and DOM helpers shared by the screens
//! (`docs/spec/dashboard.md` §7).
//!
//! Status is never conveyed by colour alone: every status pairs an icon (an
//! inline SVG from the bundled sprite, no icon CDN) with a text label, and the
//! colour is a third, redundant cue.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

use crate::viewmodel::state_class;

/// Read the `value` of the `EventTarget` of an `input` event.
pub fn value_of(event: &web_sys::Event) -> String {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|input| input.value())
        .unwrap_or_default()
}

/// Read the `value` of the `EventTarget` of a `change` event on a `<select>`.
pub fn select_value(event: &web_sys::Event) -> String {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::HtmlSelectElement>().ok())
        .map(|select| select.value())
        .unwrap_or_default()
}

/// The inline SVG icon for a status class (`ok`, `degraded`, `down`,
/// `unknown`).
#[must_use]
pub fn status_icon(state: &str) -> AnyView {
    match state_class(state) {
        "ok" => view! {
            <svg class="icon" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
                <path fill="currentColor" d="M6.5 12.2 2.8 8.5l1.1-1.1 2.6 2.6 5.5-5.5 1.1 1.1z" />
            </svg>
        }
        .into_any(),
        "degraded" => view! {
            <svg class="icon" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
                <path fill="currentColor" d="M8 1 1 14h14zM7.2 6h1.6v4H7.2zm0 5h1.6v1.6H7.2z" />
            </svg>
        }
        .into_any(),
        "down" => view! {
            <svg class="icon" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
                <path fill="currentColor" d="M3.5 2.4 8 6.9l4.5-4.5 1.1 1.1L9.1 8l4.5 4.5-1.1 1.1L8 9.1l-4.5 4.5-1.1-1.1L6.9 8 2.4 3.5z" />
            </svg>
        }
        .into_any(),
        _ => view! {
            <svg class="icon" width="16" height="16" viewBox="0 0 16 16" aria-hidden="true" focusable="false">
                <circle fill="currentColor" cx="8" cy="8" r="3" />
            </svg>
        }
        .into_any(),
    }
}

/// A status pill: icon + text + colour, always all three (`dashboard.md` §7).
#[component]
pub fn StatusBadge(#[prop(into)] state: String, #[prop(into)] label: String) -> impl IntoView {
    let class = format!("status-pill {}", state_class(&state));
    let icon = status_icon(&state);
    view! {
        <span class=class>
            {icon}
            <span>{label}</span>
        </span>
    }
}

/// An explicit empty state with the next action, never a blank panel
/// (`dashboard.md` §6).
#[component]
pub fn EmptyState(#[prop(into)] title: String, #[prop(into)] hint: String) -> impl IntoView {
    view! {
        <div class="card">
            <h3>{title}</h3>
            <p class="muted">{hint}</p>
        </div>
    }
}
