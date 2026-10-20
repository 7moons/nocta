//! The frame-rate cap on a copy of this computer's real Roblox settings.
//!
//! `cargo test -p nocta-core --test real_fps_cap -- --ignored --nocapture`
//!
//! Only a copy in a temporary folder is changed; Roblox's own file is left alone.
use nocta_core::launch::fps::{hold, read_cap, release, settings_dir, settings_file, Applied};

#[test]
#[ignore = "needs Roblox installed on this computer"]
fn real_roblox_settings_get_the_cap_and_their_own_value_back() {
    let real = settings_file(&settings_dir().expect("Roblox settings folder"))
        .expect("a GlobalBasicSettings file");
    let original_xml = std::fs::read_to_string(&real).unwrap();
    let original = read_cap(&original_xml).expect("FramerateCap in the real file");
    println!("{} has FramerateCap {original}", real.display());

    let copy = tempfile::tempdir().unwrap();
    std::fs::copy(&real, copy.path().join(real.file_name().unwrap())).unwrap();
    let state = copy.path().join("state");
    let file = || std::fs::read_to_string(settings_file(copy.path()).unwrap()).unwrap();

    assert_eq!(hold(copy.path(), &state, 60).unwrap(), Applied::Set(60));
    assert_eq!(read_cap(&file()), Some(60));
    // AFK profile: a lower cap replaces it, the user's value is still remembered.
    assert_eq!(hold(copy.path(), &state, 15).unwrap(), Applied::Set(15));
    assert_eq!(read_cap(&file()), Some(15));
    // Once the client has read it, the user's own value goes back.
    assert_eq!(
        release(copy.path(), &state).unwrap(),
        Applied::Restored(original)
    );
    assert_eq!(read_cap(&file()), Some(original));
    // Everything except the cap is exactly as Roblox wrote it.
    assert_eq!(file(), original_xml);
    assert_eq!(std::fs::read_to_string(&real).unwrap(), original_xml);
}
