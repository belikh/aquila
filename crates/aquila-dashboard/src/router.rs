//! The in-app hash router (`docs/spec/dashboard.md` §4, §6).
//!
//! Home Assistant ingress serves the panel under a token that changes per
//! session, so server-rewritten paths are unreliable. The dashboard therefore
//! routes entirely on the URL fragment (`#/rooms/<id>`): the browser never
//! sends it to the server, and it survives any ingress prefix.
//!
//! `leptos_router` 0.8 has no hash router, so this is a deliberately small one
//! over `web-sys` (`hashchange` + `location.hash`). The parsing and
//! serialisation here are pure and host-testable; the DOM glue lives in
//! `app::routing`.
//!
//! # Prose
//!
//! Australian English throughout (normalise, behaviour).

/// A resolved route (`dashboard.md` §6 screen map).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// `#/` or `#/overview`.
    Overview,
    /// `#/rooms/{id}`.
    Room {
        /// The room id (`rom_...`).
        id: String,
    },
    /// `#/structure`.
    Structure,
    /// `#/nodes`.
    Nodes,
    /// `#/pairs`.
    Pairs,
    /// `#/calibrate`.
    Calibrate,
    /// `#/health`.
    Health,
    /// `#/measurements`.
    Measurements,
    /// `#/settings`.
    Settings,
    /// An unknown fragment; rendered as an explicit empty state.
    NotFound,
}

impl Route {
    /// Parse a location hash (`#/rooms/rom_1`) into a route.
    ///
    /// Tolerant by design: an empty fragment, `#` or `#/` all resolve to
    /// [`Route::Overview`], and a `rooms` fragment with no id falls back to
    /// the structure screen rather than a blank panel.
    #[must_use]
    pub fn parse(hash: &str) -> Self {
        let path = hash.trim_start_matches('#').trim_start_matches('/');
        let mut segments = path.split('/').filter(|segment| !segment.is_empty());
        match segments.next() {
            None => Self::Overview,
            Some("overview") => Self::Overview,
            Some("rooms") => match segments.next() {
                Some(id) => Self::Room { id: id.to_string() },
                None => Self::Structure,
            },
            Some("structure") => Self::Structure,
            Some("nodes") => Self::Nodes,
            Some("pairs") => Self::Pairs,
            Some("calibrate") => Self::Calibrate,
            Some("health") => Self::Health,
            Some("measurements") => Self::Measurements,
            Some("settings") => Self::Settings,
            Some(_) => Self::NotFound,
        }
    }

    /// The canonical fragment for a route, always with a leading `#/`.
    #[must_use]
    pub fn to_hash(&self) -> String {
        match self {
            Self::Overview => "#/overview".to_string(),
            Self::Room { id } => format!("#/rooms/{id}"),
            Self::Structure => "#/structure".to_string(),
            Self::Nodes => "#/nodes".to_string(),
            Self::Pairs => "#/pairs".to_string(),
            Self::Calibrate => "#/calibrate".to_string(),
            Self::Health => "#/health".to_string(),
            Self::Measurements => "#/measurements".to_string(),
            Self::Settings => "#/settings".to_string(),
            Self::NotFound => "#/not-found".to_string(),
        }
    }

    /// The nav label for the route (the room route is not a nav entry).
    #[must_use]
    pub const fn title(&self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Room { .. } => "Room",
            Self::Structure => "Structure",
            Self::Nodes => "Nodes",
            Self::Pairs => "Node pairs",
            Self::Calibrate => "Calibration",
            Self::Health => "System health",
            Self::Measurements => "Measurements",
            Self::Settings => "Settings",
            Self::NotFound => "Not found",
        }
    }

    /// Whether this route is the active nav entry for `entry`.
    ///
    /// The room live view has no nav entry of its own; it highlights Overview,
    /// which is where the operator drills in from.
    #[must_use]
    pub fn is_entry(&self, entry: &Self) -> bool {
        match (self, entry) {
            (Self::Room { .. }, Self::Overview) => true,
            _ => self == entry,
        }
    }
}

/// The nine top-level nav entries, in screen-map order (`dashboard.md` §6).
#[must_use]
pub fn nav_entries() -> [Route; 9] {
    [
        Route::Overview,
        Route::Structure,
        Route::Nodes,
        Route::Pairs,
        Route::Calibrate,
        Route::Health,
        Route::Measurements,
        Route::Settings,
        Route::NotFound,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_fragments_resolve_to_overview() {
        assert_eq!(Route::parse(""), Route::Overview);
        assert_eq!(Route::parse("#"), Route::Overview);
        assert_eq!(Route::parse("#/"), Route::Overview);
        assert_eq!(Route::parse("#/overview"), Route::Overview);
    }

    #[test]
    fn a_room_fragment_carries_its_id() {
        assert_eq!(
            Route::parse("#/rooms/rom_abc"),
            Route::Room {
                id: "rom_abc".to_string()
            }
        );
    }

    #[test]
    fn a_room_fragment_without_an_id_falls_back_to_structure() {
        assert_eq!(Route::parse("#/rooms"), Route::Structure);
        assert_eq!(Route::parse("#/rooms/"), Route::Structure);
    }

    #[test]
    fn every_named_screen_round_trips() {
        for entry in nav_entries() {
            if entry == Route::NotFound {
                continue;
            }
            assert_eq!(Route::parse(&entry.to_hash()), entry);
        }
    }

    #[test]
    fn an_unknown_fragment_is_not_found() {
        assert_eq!(Route::parse("#/nope"), Route::NotFound);
        assert_eq!(
            Route::parse("#/rooms/a/b/c"),
            Route::Room {
                id: "a".to_string()
            }
        );
    }

    #[test]
    fn the_room_view_highlights_overview_in_the_nav() {
        let room = Route::Room {
            id: "rom_1".to_string(),
        };
        assert!(room.is_entry(&Route::Overview));
        assert!(!room.is_entry(&Route::Nodes));
    }
}
