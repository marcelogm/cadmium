use std::path::PathBuf;

use super::{TRAY_MIX, TRAY_TOOL};
use crate::smoke::SmokeConfig;
use crate::smoke::config::{SCREENSHOT, TRAY};
use crate::smoke::steps::SmokeStep;
use crate::smoke::tests::lookup;
use crate::tools::TrayTool;

#[test]
fn the_tray_tool_and_mix_follow_the_tray_and_come_before_the_screenshot() {
    let config = SmokeConfig::from_lookup(lookup(&[
        (TRAY, "1"),
        (TRAY_TOOL, "tray-mixing-knife"),
        (TRAY_MIX, "1"),
        (SCREENSHOT, "/tmp/shot.png"),
    ]))
    .unwrap();
    assert_eq!(
        config.steps(),
        [
            SmokeStep::ShowTray,
            SmokeStep::ChooseTrayTool(TrayTool::MixingKnife),
            SmokeStep::MixOnTray,
            SmokeStep::Screenshot(PathBuf::from("/tmp/shot.png")),
            SmokeStep::Quit,
        ]
    );
}

#[test]
fn a_tray_tool_alone_starts_a_smoke_run_and_an_unknown_one_is_ignored() {
    let config = SmokeConfig::from_lookup(lookup(&[(TRAY_TOOL, "tray-blender")])).unwrap();
    assert_eq!(
        config.steps(),
        [
            SmokeStep::ChooseTrayTool(TrayTool::Blender),
            SmokeStep::Quit
        ]
    );
    let config = SmokeConfig::from_lookup(lookup(&[(TRAY_TOOL, "spray")])).unwrap();
    assert_eq!(config.steps(), [SmokeStep::Quit]);
}

#[test]
fn tray_mix_zero_does_not_mix() {
    assert!(SmokeConfig::from_lookup(lookup(&[(TRAY_MIX, "0")])).is_none());
}
