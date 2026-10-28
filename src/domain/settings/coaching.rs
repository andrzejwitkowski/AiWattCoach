/// Persistent coaching policy chosen by the athlete. Three independent concepts:
/// phase (when), intensity-distribution model (how time is split), session emphasis (what quality work).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrainingPhase {
    Base,
    Build,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntensityDistribution {
    Polarized,
    Pyramidal,
    Threshold,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEmphasis {
    SweetSpot,
    Threshold,
    Vo2,
    RoadRaceRepeatability,
    AerobicEndurance,
}

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $text),+ }
            }

            pub fn parse(value: &str) -> Option<Self> {
                match value { $($text => Some(Self::$variant),)+ _ => None }
            }
        }
    };
}

string_enum!(TrainingPhase { Base => "base", Build => "build" });
string_enum!(IntensityDistribution {
    Polarized => "polarized",
    Pyramidal => "pyramidal",
    Threshold => "threshold",
});
string_enum!(SessionEmphasis {
    SweetSpot => "sweet_spot",
    Threshold => "threshold",
    Vo2 => "vo2",
    RoadRaceRepeatability => "road_race_repeatability",
    AerobicEndurance => "aerobic_endurance",
});

/// All fields unset means legacy/adaptive behaviour; the prompt must say so instead of guessing.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CoachingPreferences {
    pub phase: Option<TrainingPhase>,
    pub distribution: Option<IntensityDistribution>,
    pub emphasis: Vec<SessionEmphasis>,
}

impl CoachingPreferences {
    /// Canonical structured policy block shared by every LLM workflow.
    /// `future_target_events` must contain only real future events (`date priority discipline`).
    pub fn render_policy_context(&self, future_target_events: &[String]) -> String {
        let phase = self.phase.map_or("unset", TrainingPhase::as_str);
        let distribution = self
            .distribution
            .map_or("unset", IntensityDistribution::as_str);
        let emphasis = if self.emphasis.is_empty() {
            "unset".to_string()
        } else {
            self.emphasis
                .iter()
                .map(|value| value.as_str())
                .collect::<Vec<_>>()
                .join(",")
        };
        let events = if future_target_events.is_empty() {
            "none confirmed: do not invent a race, race TSS, race-day TSB target, taper or next-season date; \
             past events are history, not targets"
                .to_string()
        } else {
            future_target_events.join("; ")
        };
        format!(
            "coaching_policy: phase={phase}; distribution={distribution}; session_emphasis={emphasis}; \
             future_target_events={events}. \
             Precedence: saved preferences > latest explicit user correction for this plan > future calendar events \
             and availability > observed history > athlete summary > earlier assistant statements (not authoritative). \
             Unset fields mean legacy adaptive behaviour: say the selection is missing, do not guess it. \
             Phase is user-selected and never switched automatically. Name the active phase and distribution in the \
             planning rationale and keep them identical across rolling windows. Distinguish measured facts, estimates and unknowns."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_round_trip_through_strings() {
        for value in ["base", "build"] {
            assert_eq!(TrainingPhase::parse(value).unwrap().as_str(), value);
        }
        for value in ["polarized", "pyramidal", "threshold"] {
            assert_eq!(IntensityDistribution::parse(value).unwrap().as_str(), value);
        }
        for value in [
            "sweet_spot",
            "threshold",
            "vo2",
            "road_race_repeatability",
            "aerobic_endurance",
        ] {
            assert_eq!(SessionEmphasis::parse(value).unwrap().as_str(), value);
        }
        assert_eq!(TrainingPhase::parse("peak"), None);
    }

    #[test]
    fn default_is_unset_and_policy_says_so() {
        let text = CoachingPreferences::default().render_policy_context(&[]);
        assert!(text.contains("phase=unset; distribution=unset; session_emphasis=unset"));
        assert!(text.contains("do not invent a race, race TSS, race-day TSB target, taper"));
    }

    #[test]
    fn policy_names_selection_and_real_events_only() {
        let preferences = CoachingPreferences {
            phase: Some(TrainingPhase::Base),
            distribution: Some(IntensityDistribution::Polarized),
            emphasis: vec![SessionEmphasis::RoadRaceRepeatability],
        };
        let text = preferences.render_policy_context(&["2026-11-15 A road".to_string()]);
        assert!(text.contains(
            "phase=base; distribution=polarized; session_emphasis=road_race_repeatability"
        ));
        assert!(text.contains("future_target_events=2026-11-15 A road"));
        assert!(!text.contains("do not invent a race"));
    }
}
