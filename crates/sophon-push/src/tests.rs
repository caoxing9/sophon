use super::*;

fn note(kind: Kind, detail: &str) -> Note {
    Note { kind, agent: "a1".into(), title: "修登录页".into(), project: "sophon".into(), detail: detail.into(), permission: None }
}

fn wants(lang: &str) -> Wants {
    Wants { token: "ab12".into(), env: "production".into(), lang: lang.into(), ..Default::default() }
}

#[test]
fn what_is_sealed_opens_only_with_the_receiver_s_secret() {
    let (secret, public) = keypair();
    let sealed = seal(&public, "你好 hello".as_bytes());
    assert_eq!(open(&secret, &sealed).as_deref(), Some("你好 hello".as_bytes()));
    assert_eq!(open(&keypair().0, &sealed), None);
    assert_eq!(open(&secret, "short"), None);
    // Each seal is its own.
    assert_ne!(sealed, seal(&public, "你好 hello".as_bytes()));
}

#[test]
fn a_finished_turn_says_its_project_and_its_reply_s_first_words() {
    let p = note_push("mac1", &note(Kind::Finished, "The tests pass now.\nI fixed the redirect."), &wants("zh"));
    assert_eq!(p["title"], "修登录页");
    assert_eq!(p["body"], "已完成 · sophon\nThe tests pass now. I fixed the redirect.");
    assert_eq!((p["category"].as_str(), p["thread"].as_str()), (Some("FINISHED"), Some("a1")));
    assert_eq!(p["data"], json!({ "host": "mac1", "agent": "a1" }));
    let failed = note_push("mac1", &note(Kind::Failed, "exit code 101"), &wants("en"));
    assert_eq!((failed["body"].as_str(), failed["category"].as_str()), (Some("Failed · sophon\nexit code 101"), Some("ATTENTION")));
    // Nothing more to say: the state and the project.
    assert_eq!(note_push("m", &note(Kind::Finished, ""), &wants("en"))["body"], "Done · sophon");
}

#[test]
fn an_approval_names_its_command_and_keeps_its_id_for_the_actions() {
    let asking = Note { permission: Some("p1".into()), ..note(Kind::Permission, "npm  test\n--watch") };
    let p = note_push("mac1", &asking, &wants("zh-Hans"));
    assert_eq!(p["body"], "等待你的确认：npm test --watch · sophon");
    assert_eq!(p["category"], "PERMISSION");
    assert_eq!(p["data"], json!({ "host": "mac1", "agent": "a1", "permission": "p1" }));
    let q = note_push("mac1", &note(Kind::Question, "Which branch?"), &wants("en"));
    assert_eq!((q["body"].as_str(), q["category"].as_str()), (Some("Waiting for your answer: Which branch? · sophon"), Some("ATTENTION")));
}

#[test]
fn a_hidden_push_says_nothing_of_the_work() {
    let asking = Note { permission: Some("p1".into()), ..note(Kind::Permission, "rm -rf build") };
    let p = note_push("mac1", &asking, &Wants { hide: true, ..wants("en") });
    assert_eq!((p["title"].as_str(), p["body"].as_str()), (Some("Sophon"), Some("Waiting for your approval")));
    // The action still finds its permission.
    assert_eq!(p["data"]["permission"], "p1");
    let many = notes_push("m", &[note(Kind::Finished, "secret"), note(Kind::Finished, "x")], &Wants { hide: true, ..wants("en") }).unwrap();
    assert!(!many.to_string().contains("修登录页") && !many.to_string().contains("secret"));
}

#[test]
fn agents_done_at_once_are_one_push() {
    let other = Note { agent: "a2".into(), title: "写周报".into(), ..note(Kind::Failed, "boom") };
    let p = notes_push("mac1", &[note(Kind::Finished, "ok"), other], &wants("zh")).unwrap();
    assert_eq!(p["title"], "2 个 agent 有了结果");
    assert_eq!(p["body"], "修登录页\n写周报（出错）");
    assert_eq!((p["category"].as_str(), p["collapse"].as_str()), (Some("ATTENTION"), Some("finished")));
    // One alone is its own push; none, none.
    assert_eq!(notes_push("mac1", &[note(Kind::Finished, "ok")], &wants("zh")).unwrap()["thread"], "a1");
    assert!(notes_push("mac1", &[], &wants("zh")).is_none());
}

#[test]
fn a_long_body_is_cut() {
    let p = note_push("m", &note(Kind::Permission, &"x".repeat(600)), &wants("en"));
    let body = p["body"].as_str().unwrap();
    assert!(body.chars().count() <= 240);
    assert!(body.contains('…') && body.ends_with("· sophon"), "the command cut, the project kept");
}

#[test]
fn a_reply_s_excerpt_leaves_markdown_out() {
    let reply = "## Done\n\nI fixed **the redirect** in `login.rs`:\n\n```rust\nfn x() {}\n```\n\n- see [the PR](https://x.y/1)\n---\n";
    assert_eq!(excerpt(reply), "I fixed the redirect in login.rs: see the PR");
    assert_eq!(excerpt("# Done"), "Done", "a heading alone is kept");
    assert_eq!(excerpt("snake_case_name stays"), "snake_case_name stays");
}

#[test]
fn a_device_hears_only_what_it_turned_on() {
    let off = Events::from_json(Some(&json!({ "finished": false, "usage": false })));
    assert!(!off.wants(Kind::Finished) && !off.wants(Kind::Usage));
    assert!(off.wants(Kind::Permission) && off.wants(Kind::Question) && off.wants(Kind::Failed));
    // An older app names none: all.
    assert_eq!(Events::from_json(None), Events::default());
    assert!(Events::default().wants(Kind::Finished));
}

#[test]
fn the_limit_s_push_says_when_it_runs_out() {
    let p = usage_push("m", "Claude · Weekly", 80, Some("11-16 11:09"), &wants("zh"));
    assert_eq!(p["body"], "Claude · Weekly 已用 80%，按当前速度约 11-16 11:09 用完");
    assert_eq!(p["title"], "套餐额度");
    assert_eq!(usage_push("m", "Codex · Weekly", 90, None, &Wants::default())["body"], "Codex · Weekly is at 90%");
}

#[test]
fn a_limit_is_news_once_as_it_passes_each_mark() {
    assert_eq!((limit_mark(12.0), limit_mark(80.0), limit_mark(94.9), limit_mark(95.0), limit_mark(100.0)), (0, 80, 80, 95, 95));
    assert!(!limit_news(None, 80));
    assert!(limit_news(Some(0), 80));
    assert!(!limit_news(Some(80), 80));
    assert!(limit_news(Some(80), 95));
    assert!(!limit_news(Some(95), 0));
    assert!(limit_news(Some(0), 95));
}

#[test]
fn a_device_that_forgot_the_computer_is_taken_off_by_its_own_pairing_only() {
    let wants = |pairing: &str| Wants { token: "apns".into(), pairing: pairing.into(), ..Default::default() };
    let mut r = Record::default();
    r.register("phone", wants("tok-phone"));
    r.register("pad", wants("tok-pad"));
    assert!(!r.forget("phone", "tok-pad"), "another device's pairing");
    assert!(r.forget("phone", "tok-phone"));
    assert_eq!(r.devices.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["pad"]);
    let (secret, public) = keypair();
    let forget = Act::Forget { device: "phone".into() };
    let sealed = seal(&public, act_json("tok-phone", 1000, &forget).to_string().as_bytes());
    assert_eq!(check_act(&secret, &sealed, &["tok-phone".to_owned()], 1000), Ok(("tok-phone".into(), forget)));
}

#[test]
fn an_action_counts_only_from_a_paired_device_and_while_fresh() {
    let (secret, public) = keypair();
    let tokens = vec!["tok-phone".to_owned()];
    let act = Act::Permission { agent: "a1".into(), permission: "p1".into(), allow: true };
    let sealed = |token: &str, at: u64, act: &Act| seal(&public, act_json(token, at, act).to_string().as_bytes());
    assert_eq!(check_act(&secret, &sealed("tok-phone", 1000, &act), &tokens, 1100), Ok(("tok-phone".into(), act.clone())));
    assert_eq!(check_act(&secret, &sealed("tok-other", 1000, &act), &tokens, 1100), Err("not a paired device"));
    assert_eq!(check_act(&secret, &sealed("tok-phone", 1000, &act), &tokens, 1000 + 601), Err("too old"));
    assert_eq!(check_act(&keypair().0, &sealed("tok-phone", 1000, &act), &tokens, 1000), Err("not for this computer"));
    let reply = Act::Reply { agent: "a1".into(), text: "run the tests".into() };
    assert_eq!(check_act(&secret, &sealed("tok-phone", 1000, &reply), &tokens, 1000), Ok(("tok-phone".into(), reply)));
    let empty = Act::Reply { agent: "a1".into(), text: "  ".into() };
    assert_eq!(check_act(&secret, &sealed("tok-phone", 1000, &empty), &tokens, 1000), Err("incomplete action"));
}

#[test]
fn the_record_keeps_who_wants_notifications_and_the_gateway_s_first_key() {
    let mut r = Record { id: "mac1".into(), ..Default::default() };
    let w = Wants { token: "ab".into(), env: "development".into(), lang: "zh".into(), hide: true, pairing: "tok".into(), space: Some("/s/lin".into()), events: Events { finished: false, ..Events::default() } };
    assert!(r.register("dev1", w.clone()));
    assert!(!r.register("dev1", w.clone()));
    assert_eq!(Record::from_json(&r.to_json()), r);
    assert!(r.register("dev1", Wants::default()));
    assert!(r.devices.is_empty());
    r.register("dev2", w.clone());
    assert_eq!((r.wanting(Some("/s/lin")), r.wanting(None)), (vec![w.clone()], vec![]));
    assert_eq!(r.spaces(), vec![Some("/s/lin".to_owned())]);
    assert!(!r.keep_only(&["tok".to_owned()]));
    assert!(r.keep_only(&["another".to_owned()]));
    assert!(r.devices.is_empty());

    let (first, other) = ([1u8; 32], [2u8; 32]);
    assert_eq!(r.gateway_key("1.2.3.4:8443", first), Some(first));
    assert_eq!(r.gateway_key("1.2.3.4:8443", first), Some(first));
    assert_eq!(r.gateway_key("1.2.3.4:8443", other), None);
    assert_eq!(Record::from_json(&r.to_json()).gateways, vec![("1.2.3.4:8443".to_owned(), first)]);
}

#[test]
fn each_space_has_its_own_id_at_the_gateway_and_its_own_file() {
    let r = Record { id: "mac1".into(), ..Default::default() };
    assert_eq!(r.space_id(None), "mac1");
    let lin = r.space_id(Some("/Users/a/.sophon/spaces/lin"));
    assert!(lin.starts_with("mac1-") && lin != r.space_id(Some("/Users/a/.sophon/spaces/kim")));
    assert_eq!(lin, r.space_id(Some("/Users/a/.sophon/spaces/lin")), "steady");

    let home = std::env::temp_dir().join(format!("sophon-push-{}", std::process::id()));
    let file = ServiceFile { id: "mac1".into(), gateway: "1.2.3.4:8443".into(), tls: false, devices: vec![wants("zh")], secret: Some([7; 32]), tokens: vec!["tok".into()] };
    assert!(file.store(&home));
    assert!(!file.store(&home), "the same again: not written");
    assert_eq!(ServiceFile::load(&home), file);
    // A gateway's key is kept as first seen.
    assert_eq!(trust_gateway(&home, "gw", [1; 32]), Some([1; 32]));
    assert_eq!(trust_gateway(&home, "gw", [2; 32]), None);
    // A device that forgot the computer, for the app to take off.
    note_forgotten(&home, "phone", "tok");
    assert_eq!(forgotten(&home), vec![("phone".to_owned(), "tok".to_owned())]);
    clear_forgotten(&home);
    assert!(forgotten(&home).is_empty());
    // Nobody to tell: no file.
    assert!(ServiceFile::default().store(&home));
    assert_eq!(ServiceFile::load(&home), ServiceFile::default());
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn the_gateway_is_the_relay_s_server() {
    assert_eq!(gateway_url("1.2.3.4:8443", false, "id=m").as_deref(), Some("ws://1.2.3.4:8443/push?id=m"));
    assert_eq!(gateway_url("relay.me:443", true, "to=m").as_deref(), Some("wss://relay.me:443/push?to=m"));
    assert_eq!(gateway_url("relay.paseo.sh:443", true, "id=m"), None);
    assert_eq!(gateway_url("", false, "id=m"), None);
}
