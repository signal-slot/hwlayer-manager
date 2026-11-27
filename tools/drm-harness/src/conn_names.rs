//! Connector names as the kernel writes them, and matching of `--connector`.

use drm::control::connector::Interface;

/// The kernel's name of a connector type (`HDMI-A`, `DP`, `eDP`), as in
/// `/sys/class/drm/card0-HDMI-A-1`; the `Debug` text of the enum is not it.
pub fn interface_name(interface: Interface) -> String {
    use Interface as I;
    match interface {
        I::VGA => "VGA".into(),
        I::DVII => "DVI-I".into(),
        I::DVID => "DVI-D".into(),
        I::DVIA => "DVI-A".into(),
        I::SVideo => "SVIDEO".into(),
        I::NinePinDIN => "DIN".into(),
        I::DisplayPort => "DP".into(),
        I::HDMIA => "HDMI-A".into(),
        I::HDMIB => "HDMI-B".into(),
        I::EmbeddedDisplayPort => "eDP".into(),
        other => format!("{other:?}"),
    }
}

/// Whether the connector `name` is the one asked for: `want` is a prefix of
/// it, ignoring case and dashes, so `HDMI-A-1`, `HDMIA-1` and `hdmi` all
/// match `HDMI-A-1`.
pub fn connector_matches(name: &str, want: &str) -> bool {
    let squash = |text: &str| {
        text.chars()
            .filter(|c| *c != '-')
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    squash(name).starts_with(&squash(want))
}

#[cfg(test)]
mod tests {
    use super::*;
    use Interface as I;

    #[test]
    fn connectors_are_named_as_the_kernel_does() {
        assert_eq!(interface_name(I::HDMIA), "HDMI-A");
        assert_eq!(interface_name(I::DisplayPort), "DP");
        assert_eq!(interface_name(I::EmbeddedDisplayPort), "eDP");
        assert_eq!(interface_name(I::LVDS), "LVDS");
        assert_eq!(interface_name(I::DSI), "DSI");
    }

    #[test]
    fn connector_requests_match_by_prefix() {
        assert!(connector_matches("HDMI-A-1", "HDMI-A-1"));
        assert!(connector_matches("HDMI-A-1", "hdmi"));
        assert!(connector_matches("HDMI-A-1", "HDMIA-1"));
        assert!(connector_matches("DP-2", "dp"));
        assert!(connector_matches("eDP-1", "eDP"));
        assert!(!connector_matches("HDMI-A-2", "HDMI-A-1"));
        assert!(!connector_matches("DP-1", "eDP"));
    }
}
