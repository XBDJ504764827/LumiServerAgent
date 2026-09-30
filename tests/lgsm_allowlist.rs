//! LGSM 实例 allowlist 集成测试：从外部视角锁定
//! “只能执行已登记实例的三个电源动作”这一安全契约。

use lumi_server_agent::config::is_valid_instance_name;
use lumi_server_agent::lgsm::{validate_instance, PowerAction};

fn allowlist() -> Vec<String> {
    vec!["csgoserver".to_string(), "csgo2server".to_string()]
}

#[test]
fn registered_instances_pass_allowlist() {
    for name in ["csgoserver", "csgo2server"] {
        assert!(is_valid_instance_name(name));
        validate_instance(name, &allowlist()).unwrap();
    }
}

#[test]
fn injection_names_rejected_before_any_execution() {
    for evil in [
        "a;reboot",
        "$(reboot)",
        "`reboot`",
        "../csgoserver",
        "csgo|sh",
        "csgo server",
        "",
        "csgoserver\nrestart",
    ] {
        assert!(!is_valid_instance_name(evil), "正则应拦截：{}", evil);
        assert!(
            validate_instance(evil, &allowlist()).is_err(),
            "allowlist 应拦截：{}",
            evil
        );
    }
}

#[test]
fn wellformed_but_unregistered_instance_rejected() {
    assert!(is_valid_instance_name("csgo3server"));
    let err = validate_instance("csgo3server", &allowlist()).unwrap_err();
    assert!(err.to_string().contains("未在该 Agent 登记"));
}

#[test]
fn only_restart_start_stop_are_power_actions() {
    assert_eq!(PowerAction::parse("restart").unwrap(), PowerAction::Restart);
    assert_eq!(PowerAction::parse("start").unwrap(), PowerAction::Start);
    assert_eq!(PowerAction::parse("stop").unwrap(), PowerAction::Stop);
    for bad in ["status", "details", "restart now", ""] {
        assert!(PowerAction::parse(bad).is_err(), "应拒绝动作：{}", bad);
    }
}
