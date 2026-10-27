#![no_main]
use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    if data.len()!=65 {return;}
    let documents=["", "ASCII", "你好", "a🦀b", "e\u{301}"];
    let document=documents[data[0] as usize % documents.len()];
    jasper_ui_input_identity_verification::check_composition_range(document,data[1] as usize,data[2] as usize);
    let cursor=if data[3]&1==0 {None} else {Some((data[1] as usize,data[2] as usize))};
    jasper_ui_input_identity_verification::check_preedit(document,cursor);
    jasper_ui_input_identity_verification::check_desktop_ime(document,cursor);
    jasper_ui_input_identity_verification::check_normalized_preedit_pipeline(document,cursor);
    jasper_ui_input_identity_verification::check_preedit_view(document,cursor);
    jasper_ui_input_identity_verification::check_paired_preedit_cancel(document,data[4]&1!=0);
    jasper_ui_input_identity_verification::check_focus_preedit_close(document);
    jasper_ui_input_identity_verification::check_preedit_lifecycle(document);
    jasper_ui_input_identity_verification::check_input_lifecycle(document,data[5]&1!=0);
    let mut words=[0u64;8];
    for (word,bytes) in words.iter_mut().zip(data[..64].chunks_exact(8)) {*word=u64::from_le_bytes(bytes.try_into().unwrap());}
    jasper_ui_input_identity_verification::check_identity(words,data[64]);
    jasper_ui_input_identity_verification::check_binding(words,data[64]);
    jasper_ui_input_identity_verification::check_payload(words,data[64]);
    jasper_ui_input_identity_verification::check_keyboard(words,data[64]);
    jasper_ui_input_identity_verification::check_rotation(words,data[64]);
    jasper_ui_input_identity_verification::check_transfer(words,data[64]);
    jasper_ui_input_identity_verification::check_open_close(words,data[64]);
    jasper_ui_input_identity_verification::check_composition(words,data[64]);
});
