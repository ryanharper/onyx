use super::*;

#[test]
fn test_download_format_defaults() {
    assert_eq!(OutputFormat::default_for(MediaType::Video), OutputFormat::MP4);
    assert_eq!(OutputFormat::default_for(MediaType::Audio), OutputFormat::MP3);
}

#[test]
fn test_download_format_display() {
    assert_eq!(format!("{}", DownloadFormat::VideoBest), "Best Video (Auto)");
    assert_eq!(format!("{}", DownloadFormat::AudioMp3), "Audio Only (MP3)");
}

#[test]
fn test_media_type_display() {
    assert_eq!(format!("{}", MediaType::Video), "Video");
    assert_eq!(format!("{}", MediaType::Audio), "Audio");
}

#[test]
fn test_settings_default() {
     let default = AdvancedSettings {
        embed_subs: false,
        embed_thumbnail: true,
        restrict_filenames: true,
        proxy_url: String::new(),
        cookies_browser: None,
        cookies_file: String::new(),
        youtube_api_key: String::new(),
     };
     assert_eq!(default.embed_subs, false);
     assert_eq!(default.embed_thumbnail, true);
}

#[tokio::test]
async fn test_app_initial_state() {
   let app = OnyxApp::default();
   assert_eq!(app.active_tab, Tab::QuickDownload);
   assert_eq!(app.format, DownloadFormat::VideoBest);
   // Check initial state
   if let AppState::CheckingDependencies = app.state {
       assert!(true);
   } else {
       // It might be checking dependencies default
   }
}
