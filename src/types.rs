use serde::{Deserialize, Serialize};
use iced::widget::image;
use std::path::PathBuf;
use std::fs;
use directories::ProjectDirs;

// ============================================================================
// CORE ENUMS & STRUCTS
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    QuickDownload,
    BatchQueue,
    Settings,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[Tab::QuickDownload, Tab::BatchQueue, Tab::Settings]
    }
}

impl std::fmt::Display for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tab::QuickDownload => write!(f, "Download"),
            Tab::BatchQueue => write!(f, "Queue"),
            Tab::Settings => write!(f, "Settings"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Video,
    Audio,
}

impl MediaType {
    pub fn all() -> &'static [MediaType] {
        &[MediaType::Video, MediaType::Audio]
    }
}

impl std::fmt::Display for MediaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MediaType::Video => write!(f, "Video"),
            MediaType::Audio => write!(f, "Audio"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    // Video
    MP4,
    MKV,
    WEBM,
    // Audio
    MP3,
    M4A,
    OPUS,
    FLAC,
}

impl OutputFormat {
    pub fn for_media_type(media_type: MediaType) -> &'static [OutputFormat] {
        match media_type {
            MediaType::Video => &[OutputFormat::MP4, OutputFormat::MKV, OutputFormat::WEBM],
            MediaType::Audio => &[OutputFormat::MP3, OutputFormat::M4A, OutputFormat::OPUS, OutputFormat::FLAC],
        }
    }
    
    pub fn default_for(media_type: MediaType) -> Self {
        match media_type {
            MediaType::Video => OutputFormat::MP4,
            MediaType::Audio => OutputFormat::MP3,
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::MP4 => write!(f, "MP4"),
            OutputFormat::MKV => write!(f, "MKV"),
            OutputFormat::WEBM => write!(f, "WEBM"),
            OutputFormat::MP3 => write!(f, "MP3"),
            OutputFormat::M4A => write!(f, "M4A"),
            OutputFormat::OPUS => write!(f, "OPUS"),
            OutputFormat::FLAC => write!(f, "FLAC"),
        }
    }
}

// Legacy DownloadFormat for Quick Download tab
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadFormat {
    VideoBest,
    Video1080p,
    Video720p,
    AudioBest,
    AudioMp3,
}

impl DownloadFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            DownloadFormat::VideoBest => "Best Video (Auto)",
            DownloadFormat::Video1080p => "1080p",
            DownloadFormat::Video720p => "720p",
            DownloadFormat::AudioBest => "Best Audio (Auto)",
            DownloadFormat::AudioMp3 => "Audio Only (MP3)",
        }
    }
    
    pub fn all() -> &'static [DownloadFormat] {
        &[
            DownloadFormat::VideoBest, DownloadFormat::Video1080p, DownloadFormat::Video720p,
            DownloadFormat::AudioBest, DownloadFormat::AudioMp3
        ]
    }
}

impl std::fmt::Display for DownloadFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTheme {
    Default, // Cool Blue
    Vibrant, // Cyan/Gold/Black (Screenshot style)
}

#[derive(Debug, Clone)]
pub enum AppState {
    Idle,
    CheckingDependencies,
    DependencyError { error: String, downloading: bool, progress: f32 },
    Downloading { progress: f32, status_text: String },
    Finished(Result<(), String>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrimHandle {
    Start,
    End,
    Selection, // For dragging the entire selection
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrimHandleStyle;

#[derive(Debug, Clone, PartialEq)]
pub struct TrimSliderStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowseMode {
    Trending,
    SearchResults,
}

// YouTube video data
#[derive(Debug, Clone)]
pub struct YouTubeVideo {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub thumbnail_url: String,
    pub duration: String,
    pub views: String,
    pub url: String,
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub id: usize,
    pub url: String,
    pub title: Option<String>,
    pub duration: Option<f32>, // Duration in seconds
    pub thumbnail: Option<image::Handle>,
    pub media_type: MediaType,
    pub output_format: OutputFormat,
    pub time_range: Option<TimeRange>,
    pub crop_selection: Option<CropRect>,
    pub status: QueueStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CropRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimeRange {
    pub start_seconds: f32,
    pub end_seconds: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum QueueStatus {
    Fetching,
    Ready,
    Downloading(f32),
    Complete,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedSettings {
    pub embed_subs: bool,
    pub embed_thumbnail: bool,
    pub restrict_filenames: bool,
    pub proxy_url: String,
    pub cookies_browser: Option<Browser>,
    pub cookies_file: String,  // Path to cookies.txt file
    pub youtube_api_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Browser {
    Chrome, Firefox, Edge, Brave, Vivaldi, Opera, Safari, Chromium
}

impl AdvancedSettings {
    pub fn load() -> Self {
        if let Some(dirs) = ProjectDirs::from("com", "onyx", "yt-frontend") {
            let config_dir = dirs.config_dir();
            let config_file = config_dir.join("settings.json");
            
            if config_file.exists() {
                if let Ok(content) = fs::read_to_string(config_file) {
                    if let Ok(settings) = serde_json::from_str(&content) {
                        return settings;
                    }
                }
            }
        }
        
        // Default
        AdvancedSettings {
            embed_subs: false,
            embed_thumbnail: true,
            restrict_filenames: true,
            proxy_url: String::new(),
            cookies_browser: None,
            cookies_file: String::new(),
            youtube_api_key: String::new(), // Expect user input
        }
    }
    
    pub fn save(&self) {
        if let Some(dirs) = ProjectDirs::from("com", "onyx", "yt-frontend") {
            let config_dir = dirs.config_dir();
            if !config_dir.exists() {
                let _ = fs::create_dir_all(config_dir);
            }
            let config_file = config_dir.join("settings.json");
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = fs::write(config_file, json);
            }
        }
    }
}

impl Browser {
    pub const ALL: [Browser; 8] = [
        Browser::Chrome, Browser::Firefox, Browser::Edge, 
        Browser::Brave, Browser::Vivaldi, Browser::Opera, Browser::Safari, Browser::Chromium
    ];
}

impl std::fmt::Display for Browser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
