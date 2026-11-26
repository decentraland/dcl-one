//! The fleet's machines, by runner label: what a section is called and what its verdicts must
//! know about the machine.

/// The operating system of a fleet machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Windows,
}

/// A machine of the fleet.
#[derive(Debug, PartialEq, Eq)]
pub struct Machine {
    /// Its runner label.
    pub runner: &'static str,
    pub os: Os,
    /// What the report calls it (`ProduceSummary.cs`'s `labelDisplayNames`).
    pub name: &'static str,
}

static MACHINES: [Machine; 5] = [
    Machine {
        runner: "apple-m1",
        os: Os::MacOs,
        name: "Apple M1",
    },
    Machine {
        runner: "amd-ryzen-3",
        os: Os::Windows,
        name: "AMD Ryzen 3",
    },
    Machine {
        runner: "intel-i3",
        os: Os::Windows,
        name: "Intel Core i3",
    },
    Machine {
        runner: "intel-i5",
        os: Os::Windows,
        name: "Intel Core i5",
    },
    Machine {
        runner: "framework-13-i7",
        os: Os::Windows,
        name: "Framework 13 i7",
    },
];

/// A leg in Chrome on a machine is labelled `<runner label>-chrome`.
const CHROME: &str = "-chrome";

/// The fleet machine whose leg in Chrome `label` names. A label typed by hand names none.
pub fn machine_in_chrome(label: &str) -> Option<&'static Machine> {
    let runner = label.strip_suffix(CHROME)?;
    MACHINES.iter().find(|machine| machine.runner == runner)
}

/// Whether the GPU frame times of the leg `label` are unreliable: Chrome on macOS renders through
/// Metal, where a run's readings settle on one value and differ between runs of one build.
pub fn gpu_timing_unreliable(label: &str) -> bool {
    machine_in_chrome(label).is_some_and(|machine| machine.os == Os::MacOs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_names_a_machine_only_as_its_leg_in_chrome() {
        assert_eq!(
            machine_in_chrome("intel-i5-chrome").map(|machine| machine.name),
            Some("Intel Core i5")
        );
        for label in ["intel-i5", "local-chrome", "intel-i5-firefox"] {
            assert_eq!(machine_in_chrome(label), None, "{label}");
        }
    }

    #[test]
    fn gpu_timing_is_unreliable_only_in_chrome_on_a_macos_machine() {
        assert!(gpu_timing_unreliable("apple-m1-chrome"));
        for label in ["intel-i5-chrome", "apple-m1", "local-chrome"] {
            assert!(!gpu_timing_unreliable(label), "{label}");
        }
    }
}
