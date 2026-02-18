use iced::widget::{button, canvas, checkbox, column, container, image, pick_list, row, scrollable, stack, text, text_input, slider, Space, responsive, mouse_area, toggler};
use iced::{mouse, Color, Element, Length, Point, Rectangle, Subscription, Theme, Size, Task};
use iced::alignment;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader, AsyncWriteExt};
use std::process::Stdio;
use regex::Regex;
use std::time::Instant;
use iced::futures::SinkExt;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use serde::{Deserialize, Serialize};
use directories::ProjectDirs;
use std::fs;
use std::io::Write;
use lofty::prelude::*;
use lofty::tag::Tag;
use lofty::picture::{Picture, PictureType, MimeType};
use lofty::config::WriteOptions;

use iced_video_player::Video;
use iced::advanced::Widget;



pub fn main() -> iced::Result {
    iced::application(
        || (OnyxApp::default(), Task::none()),
        OnyxApp::update, 
        OnyxApp::view
    )
    .title(OnyxApp::title)
    .font(include_bytes!("../fonts/extras/ttf/Inter-Regular.ttf").as_slice())
    .font(include_bytes!("../fonts/extras/ttf/Inter-Bold.ttf").as_slice())
    .subscription(OnyxApp::subscription)
    .theme(OnyxApp::theme)
    .run()
}

fn load_icon() -> Option<iced::window::Icon> {
    // For now, return None - icon loading can be added later with proper dependencies
    // The icon.png file exists but we need to handle it differently
    None
}

// Helper to get binary directory (XDG data dir or local bin)
// This ensures compatibility with Flatpak (where app dir is read-only)
fn get_bin_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "onyx", "yt-frontend") {
        let path = proj_dirs.data_local_dir().join("bin");
        let _ = std::fs::create_dir_all(&path);
        path
    } else {
        let path = PathBuf::from("bin");
        let _ = std::fs::create_dir_all(&path);
        path
    }
}

// ============================================================================
// DATA STRUCTURES
// ============================================================================

struct OnyxApp {
    // Tab Management
    active_tab: Tab,
    tab_anim_pos: f32, // For smooth tab transition (0.0 to 1.0)
    
    // Quick Download (Tab 1)
    url: String,
    download_folder: PathBuf,
    format: DownloadFormat,
    state: AppState,
    start_time: Instant,
    thumbnail: Option<image::Handle>,
    video_duration: Option<f32>,
    quick_time_range: Option<TimeRange>,
    
    // Batch Queue (Tab 2)
    queue_url_input: String,
    download_queue: Vec<QueueItem>,
    next_queue_id: usize,
    active_downloads: usize,
    
    // YouTube Browser (NEW!)
    youtube_search_query: String,
    youtube_videos: Vec<YouTubeVideo>,
    youtube_loading: bool,
    browse_mode: BrowseMode,
    video_thumbnails: std::collections::HashMap<String, image::Handle>,
    
    // Video Player with Timeline Trimming (Apple-style)
    video_player_open: bool,
    video_player_url: String,
    video_player_title: String,
    video_player_duration: f32,
    video_player_position: f32,
    trimmer_start: f32,
    trimmer_end: f32,
    dragging_handle: Option<TrimHandle>,
    hover_handle: Option<TrimHandle>,
    video: Option<Video>,
    
    // Cropping
    video_crop_mode: bool,
    video_crop_selection: Option<CropRect>, // Normalized 0.0-1.0 relative to video frame
    crop_drag_start: Option<Point>,
     
    // Shared Settings
    show_advanced: bool,
    advanced_settings: AdvancedSettings,
    dependencies_ok: bool,
    dependency_status: String,
    started: bool,
    
    // UX State
    hovered_card: Option<String>,
    theme: AppTheme,
    editing_queue_item: Option<usize>,
}

mod types;
mod messages;

use types::*;
use messages::*;



// ============================================================================
// APPLICATION IMPLEMENTATION
// ============================================================================

impl Default for OnyxApp {
    fn default() -> Self {
        // Spawn PO Token server as background task (requires active runtime)
        tokio::spawn(async {
            start_po_token_server().await;
        });

        let default_folder = directories::UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        OnyxApp {
            active_tab: Tab::QuickDownload,
            tab_anim_pos: 0.0,
            url: String::new(),
            download_folder: default_folder,
            format: DownloadFormat::VideoBest,
            state: AppState::CheckingDependencies,
            start_time: Instant::now(),
            thumbnail: None,
            video_duration: None,
            quick_time_range: None,
            queue_url_input: String::new(),
            download_queue: Vec::new(),
            next_queue_id: 0,
            active_downloads: 0,
            show_advanced: false,
            advanced_settings: AdvancedSettings::load(),
            dependencies_ok: false,
            dependency_status: "Checking System...".to_string(),
            
            // YouTube Browser initialization
            youtube_search_query: String::new(),
            youtube_videos: Vec::new(),
            youtube_loading: false,
            browse_mode: BrowseMode::Trending,
            video_thumbnails: std::collections::HashMap::new(),
            
            // Video Player initialization
            video_player_open: false,
            video_player_url: String::new(),
            video_player_title: String::new(),
            video_player_duration: 0.0,
            video_player_position: 0.0,
            video_crop_mode: false,
            video_crop_selection: None,
            crop_drag_start: None,
            trimmer_start: 0.0,
            trimmer_end: 0.0,
            dragging_handle: None,
            hover_handle: None,
            video: None,
            started: false,
            
            hovered_card: None,
            editing_queue_item: None,
            theme: AppTheme::Default,
        }
    }
}

impl OnyxApp {
    fn title(&self) -> String {
        String::from("Onyx - YT Downloader")
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // Tab switching
            Message::SwitchTab(tab) => {
                self.active_tab = tab;
                Task::none()
            }
            
            // Dependencies
            Message::DependenciesChecked(ok, msg) => {
                self.dependencies_ok = ok;
                self.dependency_status = msg.clone();
                if ok {
                    self.state = AppState::Idle;
                } else {
                    self.state = AppState::DependencyError { error: msg, downloading: false, progress: 0.0 };
                }
                Task::none()
            }
            Message::DownloadDependencies => {
                self.state = AppState::DependencyError { 
                    error: "Downloading binaries...".to_string(), 
                    downloading: true, 
                    progress: 0.0 
                };
                Task::perform(download_dependencies_task(), Message::DependenciesDownloaded)
            }
            Message::DependenciesDownloaded(result) => {
                match result {
                    Ok(_) => {
                        self.dependencies_ok = true;
                        self.dependency_status = "Dependencies Installed.".to_string();
                        self.state = AppState::Idle;
                    },
                    Err(e) => {
                        self.dependencies_ok = false;
                        self.dependency_status = format!("Download Failed: {}", e);
                        self.state = AppState::DependencyError { error: e, downloading: false, progress: 0.0 };
                    }
                }
                Task::none()
            }
            
            // Quick Download Tab
            Message::UrlChanged(url) => {
                self.url = url.clone();
                if url.contains("youtube.com") || url.contains("youtu.be") {
                    Task::batch(vec![
                        Task::perform(fetch_thumbnail(url.clone()), Message::ThumbnailLoaded),
                        Task::perform(
                            async move {
                                match fetch_video_info(url).await {
                                    Ok((_, duration)) => Some(duration),
                                    Err(_) => None,
                                }
                            },
                            Message::VideoDurationFetched
                        ),
                    ])
                } else {
                    self.thumbnail = None;
                    self.video_duration = None;
                    self.quick_time_range = None;
                    Task::none()
                }
            }
            Message::ThumbnailLoaded(result) => {
                if let Ok(handle) = result {
                    self.thumbnail = Some(handle);
                }
                Task::none()
            }
            Message::VideoDurationFetched(duration) => {
                self.video_duration = duration;
                Task::none()
            }
            Message::ToggleQuickTimeRange(enabled) => {
                if enabled {
                    let duration = self.video_duration.unwrap_or(100.0);
                    self.quick_time_range = Some(TimeRange {
                        start_seconds: 0.0,
                        end_seconds: duration,
                    });
                } else {
                    self.quick_time_range = None;
                }
                Task::none()
            }
            Message::UpdateQuickTimeRangeStart(start) => {
                // Update video player trimmer if open
                if self.video_player_open {
                    self.trimmer_start = start.max(0.0).min(self.video_player_duration);
                    // Ensure end >= start
                    if self.trimmer_end < self.trimmer_start {
                        self.trimmer_end = self.trimmer_start;
                    }
                }
                
                // Also update quick download range if it exists
                if let Some(ref mut range) = self.quick_time_range {
                    range.start_seconds = start.max(0.0);
                    // Ensure end >= start
                    if range.end_seconds < range.start_seconds {
                        range.end_seconds = range.start_seconds;
                    }
                }
                Task::none()
            }
            Message::UpdateQuickTimeRangeEnd(end) => {
                // Update video player trimmer if open
                if self.video_player_open {
                    self.trimmer_end = end.max(0.0).min(self.video_player_duration);
                    // Ensure end >= start
                    if self.trimmer_end < self.trimmer_start {
                        self.trimmer_start = self.trimmer_end;
                    }
                }
                
                // Also update quick download range if it exists
                if let Some(ref mut range) = self.quick_time_range {
                    let max_duration = self.video_duration.unwrap_or(100.0);
                    range.end_seconds = end.min(max_duration);
                    // Ensure end >= start
                    if range.end_seconds < range.start_seconds {
                        range.start_seconds = range.end_seconds;
                    }
                }
                Task::none()
            }
            Message::FormatSelected(format) => {
                self.format = format;
                Task::none()
            }
            Message::DownloadPressed => {
                if !self.url.trim().is_empty() {
                    self.state = AppState::Downloading { progress: 0.0, status_text: "Initializing...".to_string() };
                }
                Task::none()
            }
            Message::DownloadProgress(event) => {
                match event {
                    DownloadEvent::Starting => {
                        self.state = AppState::Downloading { progress: 0.0, status_text: "Downloading...".to_string() };
                    }
                    DownloadEvent::Progress(p, text) => {
                        if let AppState::Downloading { .. } = &mut self.state {
                            self.state = AppState::Downloading { progress: p, status_text: text };
                        }
                    }
                    DownloadEvent::Finished(result) => {
                        self.state = AppState::Finished(result);
                    }
                }
                Task::none()
            }
            
            // Batch Queue Tab
            Message::QueueUrlInputChanged(url) => {
                self.queue_url_input = url;
                Task::none()
            }
            Message::AddToQueue => {
                if !self.queue_url_input.trim().is_empty() {
                    let id = self.next_queue_id;
                    self.next_queue_id += 1;
                    
                    let item = QueueItem {
                        id,
                        url: self.queue_url_input.clone(),
                        title: None,
                        duration: None,
                        thumbnail: None,
                        media_type: MediaType::Video,
                        output_format: OutputFormat::MP4,
                        time_range: None,
                        crop_selection: None,
                        status: QueueStatus::Fetching,
                    };
                    
                    self.download_queue.push(item);
                    self.queue_url_input.clear();
                    
                    // Fetch info and thumbnail
                    let url = self.download_queue.last().unwrap().url.clone();
                    Task::batch(vec![
                        Task::perform(fetch_video_info(url.clone()), move |result| {
                            Message::QueueItemInfoFetched(id, result)
                        }),
                        Task::perform(fetch_thumbnail(url), move |result| {
                            Message::QueueItemThumbnailLoaded(id, result)
                        }),
                    ])
                } else {
                    Task::none()
                }
            }
            Message::RemoveQueueItem(id) => {
                self.download_queue.retain(|item| item.id != id);
                Task::none()
            }
            Message::MoveQueueItemUp(id) => {
                if let Some(pos) = self.download_queue.iter().position(|item| item.id == id) {
                    if pos > 0 {
                        self.download_queue.swap(pos, pos - 1);
                    }
                }
                Task::none()
            }
            Message::MoveQueueItemDown(id) => {
                if let Some(pos) = self.download_queue.iter().position(|item| item.id == id) {
                    if pos < self.download_queue.len() - 1 {
                        self.download_queue.swap(pos, pos + 1);
                    }
                }
                Task::none()
            }
            
            // Queue item configuration
            Message::UpdateQueueItemMediaType(id, media_type) => {
                if let Some(item) = self.download_queue.iter_mut().find(|i| i.id == id) {
                    item.media_type = media_type;
                    item.output_format = OutputFormat::default_for(media_type);
                }
                Task::none()
            }
            Message::UpdateQueueItemFormat(id, format) => {
                if let Some(item) = self.download_queue.iter_mut().find(|i| i.id == id) {
                    item.output_format = format;
                }
                Task::none()
            }
            Message::ToggleQueueItemTimeRange(id, enabled) => {
                if let Some(item) = self.download_queue.iter_mut().find(|i| i.id == id) {
                    if enabled {
                        let duration = item.duration.unwrap_or(100.0);
                        item.time_range = Some(TimeRange {
                            start_seconds: 0.0,
                            end_seconds: duration,
                        });
                    } else {
                        item.time_range = None;
                    }
                }
                Task::none()
            }
            Message::UpdateQueueItemTimeRangeStart(id, start) => {
                if let Some(item) = self.download_queue.iter_mut().find(|i| i.id == id) {
                    if let Some(ref mut range) = item.time_range {
                        range.start_seconds = start.max(0.0);
                        // Ensure end >= start
                        if range.end_seconds < range.start_seconds {
                            range.end_seconds = range.start_seconds;
                        }
                    }
                }
                Task::none()
            }
            Message::UpdateQueueItemTimeRangeEnd(id, end) => {
                if let Some(item) = self.download_queue.iter_mut().find(|i| i.id == id) {
                    if let Some(ref mut range) = item.time_range {
                        let max_duration = item.duration.unwrap_or(100.0);
                        range.end_seconds = end.min(max_duration);
                        // Ensure end >= start
                        if range.end_seconds < range.start_seconds {
                            range.start_seconds = range.end_seconds;
                        }
                    }
                }
                Task::none()
            }
            
            // Queue item info fetching
            Message::QueueItemInfoFetched(id, result) => {
                if let Some(item) = self.download_queue.iter_mut().find(|i| i.id == id) {
                    match result {
                        Ok((title, duration)) => {
                            item.title = Some(title);
                            item.duration = Some(duration);
                            item.status = QueueStatus::Ready;
                        }
                        Err(e) => {
                            item.status = QueueStatus::Failed(e);
                        }
                    }
                }
                Task::none()
            }
            Message::QueueItemThumbnailLoaded(id, result) => {
                if let Some(item) = self.download_queue.iter_mut().find(|i| i.id == id) {
                    if let Ok(handle) = result {
                        item.thumbnail = Some(handle);
                    }
                }
                Task::none()
            }
            
            // Batch download
            Message::StartBatchDownload => {
                // Start downloads for all ready items (max 20 parallel)
                let ready_items: Vec<_> = self.download_queue.iter()
                    .filter(|item| matches!(item.status, QueueStatus::Ready))
                    .take(20)
                    .map(|item| item.clone())
                    .collect();
                
                let commands: Vec<_> = ready_items.iter().map(|item| {
                    let id = item.id;
                    let item_clone = item.clone();
                    let folder = self.download_folder.clone();
                    let settings = self.advanced_settings.clone();
                    
                    Task::perform(
                        download_queue_item(item_clone, folder, settings),
                        move |result| Message::QueueItemDownloadComplete(id, result)
                    )
                }).collect();
                
                // Mark items as downloading
                for item in &ready_items {
                    if let Some(queue_item) = self.download_queue.iter_mut().find(|i| i.id == item.id) {
                        queue_item.status = QueueStatus::Downloading(0.0);
                    }
                }
                
                self.active_downloads = ready_items.len();
                Task::batch(commands)
            }
            Message::QueueItemDownloadProgress(id, progress, _status) => {
                if let Some(item) = self.download_queue.iter_mut().find(|i| i.id == id) {
                    item.status = QueueStatus::Downloading(progress);
                }
                Task::none()
            }
            Message::QueueItemDownloadComplete(id, result) => {
                if let Some(item) = self.download_queue.iter_mut().find(|i| i.id == id) {
                    match result {
                        Ok(_) => item.status = QueueStatus::Complete,
                        Err(e) => item.status = QueueStatus::Failed(e),
                    }
                }
                self.active_downloads = self.active_downloads.saturating_sub(1);
                Task::none()
            }
            
            // Shared
            Message::BrowseFolder => {
                let current_dir = self.download_folder.clone();
                Task::perform(async move {
                    rfd::AsyncFileDialog::new().set_directory(&current_dir).pick_folder().await.map(|h| h.path().to_path_buf())
                }, Message::FolderSelected)
            }
            Message::FolderSelected(path) => {
                if let Some(p) = path {
                    self.download_folder = p;
                }
                Task::none()
            }
            Message::Tick(_) => {
                // Animate tab position
                let target = match self.active_tab {
                    Tab::QuickDownload => 0.0,
                    Tab::BatchQueue => 1.0,
                    Tab::Settings => 2.0,
                };
                let diff = target - self.tab_anim_pos;
                if diff.abs() > 0.001 {
                    self.tab_anim_pos += diff * 0.2; // Smooth ease-out
                } else {
                    self.tab_anim_pos = target;
                }

                if !self.started {
                    self.started = true;
                    Task::batch(vec![
                        Task::perform(check_dependencies(), |(ok, msg)| Message::DependenciesChecked(ok, msg)),
                        Task::perform(async {}, |_| Message::LoadTrendingVideos),
                    ])
                } else {
                    Task::none()
                }
            }
            
            // Advanced
            Message::ToggleAdvanced => {
                self.show_advanced = !self.show_advanced;
                Task::none()
            }
            Message::YouTubeApiKeyChanged(key) => {
                self.advanced_settings.youtube_api_key = key;
                self.advanced_settings.save();
                Task::none()
            }
            
            Message::ToggleEmbedSubs(val) => {
                self.advanced_settings.embed_subs = val;
                self.advanced_settings.save();
                Task::none()
            }
            Message::ToggleEmbedThumbnail(val) => {
                self.advanced_settings.embed_thumbnail = val;
                self.advanced_settings.save();
                Task::none()
            }
            Message::ToggleRestrictFilenames(val) => {
                self.advanced_settings.restrict_filenames = val;
                self.advanced_settings.save();
                Task::none()
            }
            Message::ProxyChanged(val) => {
                self.advanced_settings.proxy_url = val;
                self.advanced_settings.save();
                Task::none()
            }
            Message::BrowserSelected(browser) => {
                self.advanced_settings.cookies_browser = Some(browser);
                self.advanced_settings.save();
                Task::none()
            }
            Message::ClearBrowserCookies => {
                self.advanced_settings.cookies_browser = None;
                self.advanced_settings.save();
                Task::none()
            }

            
            // YouTube Browser
            Message::YouTubeSearchQueryChanged(query) => {
                self.youtube_search_query = query;
                Task::none()
            }
            Message::YouTubeSearchSubmitted => {
                if self.youtube_search_query.is_empty() {
                    return Task::none();
                }
                self.youtube_loading = true;
                self.browse_mode = BrowseMode::SearchResults;
                let query = self.youtube_search_query.clone();
                let api_key = self.advanced_settings.youtube_api_key.clone();
                Task::perform(search_youtube(query, api_key), Message::YouTubeVideosLoaded)
            }
            Message::LoadTrendingVideos => {
                self.youtube_loading = true;
                self.browse_mode = BrowseMode::Trending;
                let api_key = self.advanced_settings.youtube_api_key.clone();
                Task::perform(load_trending_videos(api_key), Message::YouTubeVideosLoaded)
            }
            Message::YouTubeVideosLoaded(videos) => {
                self.youtube_loading = false;
                self.youtube_videos = videos.clone();
                
                // Load thumbnails for all videos
                let commands: Vec<_> = videos.iter().map(|video| {
                    let url = video.thumbnail_url.clone();
                    let id = video.id.clone();
                    Task::perform(load_youtube_thumbnail(url), move |handle| {
                        Message::YouTubeThumbnailLoaded(id.clone(), handle)
                    })
                }).collect();
                
                Task::batch(commands)
            }
            Message::YouTubeThumbnailLoaded(id, handle) => {
                self.video_thumbnails.insert(id, handle);
                Task::none()
            }
            Message::AddYouTubeVideoToQueue(video) => {
                // Add video to download queue
                let item = QueueItem {
                    id: self.next_queue_id,
                    url: video.url.clone(),
                    title: Some(video.title.clone()),
                    duration: parse_duration(&video.duration),
                    thumbnail: self.video_thumbnails.get(&video.id).cloned(),
                    media_type: MediaType::Video,
                    output_format: OutputFormat::MP4,
                    time_range: None,
                    crop_selection: None,
                    status: QueueStatus::Ready,
                };
                self.download_queue.push(item);
                self.next_queue_id += 1;
                
                // Switch to batch queue tab to show the added video
                self.active_tab = Tab::BatchQueue;
                Task::none()
            }
            Message::SwitchBrowseMode(mode) => {
                self.browse_mode = mode;
                match mode {
                    BrowseMode::Trending => Task::perform(async {}, |_| Message::LoadTrendingVideos),
                    BrowseMode::SearchResults => Task::none(),
                }
            }
            
            // Video Player Messages
            Message::OpenVideoPlayer(url, title, duration) => {
                self.video_player_open = true;
                self.video_player_url = url.clone();
                self.video_player_title = title;
                self.video_player_duration = duration;
                self.video_player_position = 0.0;
                self.trimmer_start = 0.0;
                self.trimmer_end = duration;
                self.dragging_handle = None;
                self.hover_handle = None;
                self.video = None; // Reset video state
                
                // Resolve stream URL using yt-dlp
                let settings = self.advanced_settings.clone();
                Task::perform(resolve_stream_url(url, settings), Message::VideoUrlResolved)
            }

            Message::VideoUrlResolved(result) => {
                match result {
                    Ok((stream_url, duration)) => {
                        println!("Resolved stream URL: {}, Duration: {}", stream_url, duration);
                        self.video_player_duration = duration;
                        self.trimmer_end = duration;
                        
                        if let Ok(parsed) = reqwest::Url::parse(&stream_url) {
                            match Video::new(&parsed) {
                                Ok(video) => self.video = Some(video),
                                Err(e) => eprintln!("Failed to initialize video: {:?}", e),
                            }
                        } else {
                            eprintln!("Invalid stream URL: {}", stream_url);
                        }
                    }
                    Err(e) => {
                         eprintln!("Stream resolution failed: {}", e);
                    }
                }
                Task::none()
            }
            
            Message::CloseVideoPlayer => {
                self.video_player_open = false;
                // Stop mpv playback if it was started
                self.video = None;
                Task::none()
            }
            
            Message::UpdatePlayerPosition(position) => {
                self.video_player_position = position.max(0.0).min(self.video_player_duration);
                Task::none()
            }
            
            Message::TrimHandlePressed(handle, _mouse_x) => {
                self.dragging_handle = Some(handle);
                Task::none()
            }
            
            Message::TrimHandleDragged(mouse_x) => {
                if let Some(handle) = self.dragging_handle {
                    // Convert mouse_x to time position (assuming timeline width of 800px)
                    let timeline_width = 800.0;
                    let time_position = (mouse_x / timeline_width) * self.video_player_duration;
                    let time_position = time_position.max(0.0).min(self.video_player_duration);
                    
                    match handle {
                        TrimHandle::Start => {
                            let new_start = time_position.min(self.trimmer_end - 1.0);
                            self.trimmer_start = new_start;
                            // Seek video to new start position for preview
                            self.video_player_position = new_start;
                            if let Some(video) = &mut self.video {
                                let _ = video.seek(std::time::Duration::from_secs_f32(new_start), false);
                            }
                        }
                        TrimHandle::End => {
                            self.trimmer_end = time_position.max(self.trimmer_start + 1.0);
                        }
                        TrimHandle::Selection => {
                            // Move entire selection
                            let duration = self.trimmer_end - self.trimmer_start;
                            self.trimmer_start = time_position;
                            self.trimmer_end = (time_position + duration).min(self.video_player_duration);
                        }
                    }
                }
                Task::none()
            }
            
            Message::TrimHandleReleased => {
                self.dragging_handle = None;
                Task::none()
            }
            
            Message::TrimHandleHover(handle) => {
                self.hover_handle = handle;
                Task::none()
            }
            
            Message::SeekToPosition(position) => {
                self.video_player_position = position.max(0.0).min(self.video_player_duration);
                // TODO: Send seek command to mpv
                Task::none()
            }
            
            Message::ToggleCropMode => {
                self.video_crop_mode = !self.video_crop_mode;
                // If exiting crop mode, optionally clear or keep selection?
                // For now, keep it so user can verify.
                Task::none()
            }
            Message::StartCropDrag(p) => {
                self.crop_drag_start = Some(p);
                self.video_crop_selection = None;
                Task::none()
            }
            Message::UpdateCropDrag(p) => {
                if let Some(start) = self.crop_drag_start {
                    let x = start.x.min(p.x).max(0.0).min(1.0);
                    let y = start.y.min(p.y).max(0.0).min(1.0);
                    let width = (start.x - p.x).abs().min(1.0 - x); // Clamp width
                    let height = (start.y - p.y).abs().min(1.0 - y); // Clamp height
                    
                    if width > 0.01 && height > 0.01 {
                        self.video_crop_selection = Some(CropRect { x, y, width, height });
                    }
                }
                Task::none()
            }
            Message::EndCropDrag(_) => {
                self.crop_drag_start = None;
                Task::none()
            }

            // UX Messages
            Message::CardHovered(id) => {
                self.hovered_card = Some(id);
                Task::none()
            }
            Message::CardUnhovered => {
                self.hovered_card = None;
                Task::none()
            }
            Message::EditQueueItem(id) => {
                if let Some(item) = self.download_queue.iter().find(|i| i.id == id) {
                    self.editing_queue_item = Some(id);
                    self.video_player_open = true;
                    self.video_player_url = item.url.clone();
                    self.video_player_title = item.title.clone().unwrap_or_default();
                    self.video_player_duration = item.duration.unwrap_or(0.0);
                    self.video_player_position = 0.0;
                    
                    if let Some(range) = &item.time_range {
                        self.trimmer_start = range.start_seconds;
                        self.trimmer_end = range.end_seconds;
                    } else {
                        self.trimmer_start = 0.0;
                        self.trimmer_end = item.duration.unwrap_or(0.0);
                    }
                    
                    self.video_crop_selection = item.crop_selection.clone();
                    self.video_crop_mode = item.crop_selection.is_some();
                    
                    self.video = None;
                    self.dragging_handle = None;
                    self.hover_handle = None;
                    
                    let settings = self.advanced_settings.clone();
                    Task::perform(resolve_stream_url(item.url.clone(), settings), Message::VideoUrlResolved)
                } else {
                    Task::none()
                }
            }
            Message::UpdateQueueItem => {
                if let Some(id) = self.editing_queue_item {
                    if let Some(item) = self.download_queue.iter_mut().find(|i| i.id == id) {
                        let full_duration = item.duration.unwrap_or(self.video_player_duration);
                        // Update Time Range
                        if self.trimmer_start > 0.1 || self.trimmer_end < full_duration - 0.1 {
                             item.time_range = Some(TimeRange {
                                 start_seconds: self.trimmer_start,
                                 end_seconds: self.trimmer_end,
                             });
                        } else {
                             item.time_range = None;
                        }
                        // Update Crop
                        item.crop_selection = self.video_crop_selection.clone();
                    }
                }
                self.video_player_open = false;
                self.video = None;
                self.editing_queue_item = None;
                Task::none()
            }

            Message::AddTrimmedToQueue => {
                // Add the trimmed video to the queue
                let item = QueueItem {
                    id: self.next_queue_id,
                    url: self.video_player_url.clone(),
                    title: Some(self.video_player_title.clone()),
                    duration: Some(self.video_player_duration),
                    thumbnail: None,
                    media_type: MediaType::Video,
                    output_format: OutputFormat::MP4,
                    time_range: Some(TimeRange {
                        start_seconds: self.trimmer_start,
                        end_seconds: self.trimmer_end,
                    }),
                    crop_selection: self.video_crop_selection.clone(), // Include crop
                    status: QueueStatus::Ready,
                };
                
                self.download_queue.push(item);
                self.next_queue_id += 1;
                self.video_player_open = false; // Close player after adding
                
                self.video_player_open = false; // Close player after adding
                
                Task::none()
            }
            
            Message::SwitchTheme => {
                self.theme = match self.theme {
                    AppTheme::Default => AppTheme::Vibrant,
                    AppTheme::Vibrant => AppTheme::Default,
                };
                Task::none()
            }

        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let tick = iced::window::frames().map(|_| Message::Tick(()));
        
        let download = match &self.state {
            AppState::Downloading { .. } => {
                download_subscription(
                    self.url.clone(), 
                    self.download_folder.clone(), 
                    self.format, 
                    self.advanced_settings.clone(),
                    self.quick_time_range.clone()
                )
            }
            _ => Subscription::none(),
        };

        // Video player subscription
        // Note: iced_video_player generally handles its own subscription or uses internal events
        // We add this if the library exposes a subscription method.
        // For now, we assume standard widget behavior or no subscription needed for basic playback
        // If compilation fails, we will adjust.
        let video_sub = Subscription::none();

        Subscription::batch(vec![download, tick, video_sub])
    }

    fn view(&self) -> Element<'_, Message> {
        // Show dependency error screen if needed
        if let AppState::DependencyError { error, downloading, progress } = &self.state {
            return self.view_dependency_error(error, *downloading, *progress);
        }
        
        // Header with logo and status
        let logo = canvas(AnimatedLogo { tick: self.start_time.elapsed().as_secs_f32() })
            .width(Length::Fixed(60.0))
            .height(Length::Fixed(60.0));
            
        let title = column![
            text("ONYX").size(30).font(iced::Font { weight: iced::font::Weight::Bold, ..Default::default() }),
            text("Downloader").size(14).style(|_| text::Style { color: Some(Color::from_rgb(0.7,0.7,0.7)), ..Default::default() })
        ];
        
        let theme_btn = button(text(if self.theme == AppTheme::Default { "🎨" } else { "☀️" }).size(20))
            .on_press(Message::SwitchTheme)
            .padding(10)
            .style(move |_, status| glass_secondary_style(status, self.theme));

        let header = row![
            logo,
            horizontal_space().width(20),
            title,
            horizontal_space(),
            // Status pill
             container(row![
                 text(if self.dependencies_ok { "✓ SYSTEM READY" } else { "⚠ MISSING DEPS" }).size(12)
                     .style(|_| text::Style { color: Some(if self.dependencies_ok { Color::from_rgb(0.4, 1.0, 0.4) } else { Color::from_rgb(1.0, 0.4, 0.4) }), ..Default::default() }),
             ].align_y(iced::alignment::Vertical::Center))
             .padding([6, 12])
             .style(move |_| container::Style {
                 background: Some(iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.3))),
                 border: iced::border::Border { 
                     color: Color::from_rgba(1.0, 1.0, 1.0, 0.1), 
                     width: 1.0, 
                     radius: 20.0.into() 
                 },
                 ..Default::default()
             }),
             horizontal_space().width(10),
             theme_btn
        ]
        .align_y(iced::alignment::Vertical::Center);

            

        
        // Animated Tab Bar
        let tabs = container(
            canvas(AnimatedTabBar { 
                active_tab: self.active_tab, 
                anim_pos: self.tab_anim_pos,
                queue_count: self.download_queue.len()
            })
            .width(Length::Fixed(420.0))
            .height(Length::Fixed(50.0))
        )
        .padding(10);
        
        // LEFT PANEL: Downloader (existing functionality)
        let left_panel = column![
            container(tabs).width(Length::Fill).style(move |_| card_style(self.theme)),
            match self.active_tab {
                Tab::QuickDownload => self.view_quick_download(),
            Tab::BatchQueue => self.view_batch_queue(),
            Tab::Settings => self.view_settings(),
        }
        ]
        .spacing(10)
        .width(Length::Fixed(450.0)); // Fixed width for downloader panel to maximize video space
        
        // RIGHT PANEL: YouTube Browser
        let right_panel = self.view_youtube_browser();
        
        // SPLIT SCREEN LAYOUT
        let main_content = column![
            header,
            row![
                left_panel,
                horizontal_space().width(10),
                right_panel
            ]
            .height(Length::Fill)
        ]
        .spacing(10)
        .padding(10);
        
        // Wrap with video player modal if open
        if self.video_player_open {
            let player_modal = self.view_video_player();
            
            // Show player modal centered on dark overlay
            container(
                container(player_modal)
                    .width(Length::Fixed(960.0)) // Fixed width for window feel
                    .style(|_| {
                        container::Style {
                            background: Some(iced::Background::Color(Color::from_rgba(0.15, 0.15, 0.2, 0.9))),
                            border: iced::border::Border {
                                color: Color::from_rgb(0.3, 0.3, 0.35),
                                width: 1.0,
                                radius: 12.0.into(),
                            },
                            shadow: iced::Shadow {
                                color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
                                offset: iced::Vector::new(0.0, 10.0),
                                blur_radius: 20.0,
                            },
                            text_color: Some(Color::WHITE),
                            snap: false,
                        }
                    })
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(|_| {
                container::Style {
                     background: Some(iced::Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.75))),
                     ..Default::default()
                }
            })
            .into()
        } else {
            container(main_content)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| main_background_style(self.theme))
                .into()
        }
    }
    
    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

impl OnyxApp {
    fn view_youtube_browser(&self) -> Element<'_, Message> {
        // Search bar
        let search_bar = row![
            text_input("Search YouTube...", &self.youtube_search_query)
                .on_input(Message::YouTubeSearchQueryChanged)
                .on_submit(Message::YouTubeSearchSubmitted)
                .padding(12)
                .width(Length::Fill)
                .style(rounded_text_input_style),
            button(
                row![
                    text("🔍").size(16),
                    text("Search").size(16)
                ].spacing(8).align_y(iced::alignment::Vertical::Center)
            )
                .on_press(Message::YouTubeSearchSubmitted)
                .padding(12)
                .style(move |_, s| glass_primary_style(s, self.theme)),
        ]
        .spacing(10);
        
        // Browse mode tabs
        let mode_tabs = row![
            button(
                row![
                    text("🔥").size(16),
                    text("Trending").size(16)
                ].spacing(8).align_y(iced::alignment::Vertical::Center)
            )
                .padding(10)
                .style(move |_, s| if self.browse_mode == BrowseMode::Trending {
                    glass_primary_style(s, self.theme)
                } else {
                    glass_secondary_style(s, self.theme)
                })
                .on_press(Message::SwitchBrowseMode(BrowseMode::Trending)),
            button(
                row![
                    text("📋").size(16),
                    text("Search Results").size(16)
                ].spacing(8).align_y(iced::alignment::Vertical::Center)
            )
                .padding(10)
                .style(move |_, s| if self.browse_mode == BrowseMode::SearchResults {
                    glass_primary_style(s, self.theme)
                } else {
                    glass_secondary_style(s, self.theme)
                })
                .on_press(Message::SwitchBrowseMode(BrowseMode::SearchResults)),
        ]
        .spacing(10);
        
        // Responsive Video Grid
        let video_grid: Element<'_, Message> = if self.youtube_loading {
             container(text("Loading videos...").size(18))
                .width(Length::Fill)
                .height(Length::Fixed(300.0))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        } else if self.youtube_videos.is_empty() {
             container(text("No videos found. Try searching or check API key.").size(18))
                .width(Length::Fill)
                .height(Length::Fixed(300.0))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into()
        } else {
             responsive(move |size| {
                 let card_width = 320.0;
                 let spacing = 20.0;
                 let visible_width = size.width - 40.0; // margins
                 let cols = (visible_width / (card_width + spacing)).floor().max(1.0) as usize;
                 
                 let rows: Vec<Element<Message>> = self.youtube_videos.chunks(cols).map(|chunk| {
                     let mut row_content = row![].spacing(spacing);
                     for video in chunk {
                         row_content = row_content.push(self.view_youtube_video_card(video));
                     }
                     row_content.into()
                 }).collect();
                 
                 scrollable(
                     column(rows)
                        .spacing(spacing)
                        .padding(20)
                        .width(Length::Fill)
                 )
                 .into()
             }).into()
        };

        let content: Element<'_, Message> = column![
            search_bar,
            mode_tabs,
            vertical_space().height(10),
            video_grid
        ]
        .spacing(15)
        .padding(20)
        .into();
        
        container(content)
            .padding(15)
            .style(move |_| card_style(self.theme))
            .into()
    }
    
    fn view_youtube_video_card<'a>(&'a self, video: &'a YouTubeVideo) -> Element<'a, Message> {
        let duration_secs = parse_duration_to_seconds(&video.duration);
        
        // Increased Thumbnail Size & Quality
        let thumbnail: Element<'a, Message> = if let Some(handle) = self.video_thumbnails.get(&video.id) {
             container(
                image::<iced::widget::image::Handle>(handle.clone())
                    .width(Length::Fill)
                    .height(Length::Fixed(180.0)) // Taller thumbnail
                    .content_fit(iced::ContentFit::Cover)
             )
             .width(Length::Fill)
             .height(Length::Fixed(180.0))
             .style(|_| container::Style {
                 border: iced::border::Border {
                     width: 0.0,
                     radius: iced::border::Radius {
                         top_left: 12.0,
                         top_right: 12.0,
                         bottom_left: 0.0,
                         bottom_right: 0.0,
                     },
                     color: Color::TRANSPARENT,
                 },
                 ..Default::default()
             })
             .into()
        } else {
             // Stylish Placeholder
             container(text("No Image").size(20).style(|_| text::Style { color: Some(Color::from_rgb(0.5, 0.5, 0.5)), ..Default::default() }))
                .width(Length::Fill)
                .height(Length::Fixed(180.0))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style::default().background(Color::from_rgb(0.1, 0.1, 0.1)))
                .into()
        };

        // Play Button Overlay
        let play_overlay = container(
            text("▶")
                .size(50)
                .style(|_| text::Style { color: Some(Color::WHITE.into()), ..Default::default() })
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill);
        
        // Thumbnail Stack Button
        let thumbnail_btn = button(
             stack![
                 thumbnail,
                 play_overlay
             ]
        )
        .padding(0)
        .style(move |_, s| glass_secondary_style(s, self.theme)) // Transparent/Minimal style
        .on_press(Message::OpenVideoPlayer(video.url.clone(), video.title.clone(), duration_secs));
        
        // Info Section
        let title = text(&video.title).size(16).font(iced::Font { weight: iced::font::Weight::Bold, ..Default::default() }).style(|_| text::Style { color: Some(Color::WHITE), ..Default::default() });
        let channel = text(&video.channel).size(12).style(|_| text::Style { color: Some(Color::from_rgb(0.9, 0.9, 1.0)), ..Default::default() });
        let details = text(format!("👁 {} • ⏱ {}", video.views, video.duration)).size(12).style(|_| text::Style { color: Some(Color::from_rgb(0.7, 0.7, 0.7)), ..Default::default() });
        
        let add_btn = button(
             container(
                 row![
                     text("➕").size(14),
                     text("Add to Queue").size(14)
                 ].spacing(8).align_y(iced::alignment::Vertical::Center)
             )
             .width(Length::Fill)
             .align_x(iced::alignment::Horizontal::Center)
        )
            .padding(10)
            .width(Length::Fill)
            .style(move |_, s| glass_primary_style(s, self.theme))
            .on_press(Message::AddYouTubeVideoToQueue(video.clone()));

        // Card Layout
        let container_card = container(
            column![
                thumbnail_btn,
                container(
                    column![
                        row![
                            title,
                            horizontal_space()
                        ]
                        .align_y(iced::alignment::Vertical::Top)
                        ,
                        vertical_space().height(8),
                        row![
                             container(channel)
                                .padding([4, 8])
                                .style(|_| container::Style {
                                    background: Some(iced::Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.1))),
                                    border: iced::border::Border {
                                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.2),
                                        width: 1.0,
                                        radius: 4.0.into(),
                                    },
                                    ..Default::default()
                                }),
                             horizontal_space(),
                             details
                        ]
                        .align_y(iced::alignment::Vertical::Center),
                        
                        vertical_space().height(12),
                        add_btn
                    ]
                ).padding(12)
            ]
        )
        .width(Length::Fixed(320.0)); // Slightly wider card
        
        let is_hovered = self.hovered_card.as_deref() == Some(&video.id);
        
        // Dynamic Style
        let card_container = container_card
        .style(move |_| {
             let base_color = Color::from_rgba(0.18, 0.18, 0.22, 0.95);
             let hover_color = Color::from_rgba(0.22, 0.22, 0.28, 0.98);
             let border_color = if is_hovered { Color::from_rgba(0.6, 0.3, 0.9, 0.6) } else { Color::from_rgba(1.0, 1.0, 1.0, 0.1) };
             
             container::Style {
                 background: Some(iced::Background::Color(if is_hovered { hover_color } else { base_color })),
                 border: iced::border::Border {
                     color: border_color,
                     width: if is_hovered { 2.0 } else { 1.0 },
                     radius: 12.0.into(),
                 },
                 shadow: iced::Shadow {
                     color: if is_hovered { Color::from_rgba(0.6, 0.3, 0.9, 0.4) } else { Color::from_rgba(0.0, 0.0, 0.0, 0.4) },
                     offset: iced::Vector::new(0.0, 4.0),
                     blur_radius: 12.0,
                 },
                 ..Default::default()
             }
        });
        
        mouse_area(card_container)
            .on_enter(Message::CardHovered(video.id.clone()))
            .on_exit(Message::CardUnhovered)
            .into()
    }
}

// Continue in next part...

// ============================================================================
// VIEW IMPLEMENTATIONS
// ============================================================================

impl OnyxApp {
    fn view_settings(&self) -> Element<'_, Message> {
        let content = column![
            row![
                text("⚙️").size(28),
                text("Settings").size(28).font(iced::Font{weight: iced::font::Weight::Bold, ..Default::default()})
            ].spacing(10).align_y(iced::alignment::Vertical::Center),
            
            vertical_space().height(30),
            
            // API Key Section
            container(
                column![
                    row![
                        text("🔑").size(20),
                        text("YouTube API Configuration").size(18).font(iced::Font{weight: iced::font::Weight::Bold, ..Default::default()})
                    ].spacing(10).align_y(iced::alignment::Vertical::Center),
                    
                    vertical_space().height(15),
                    
                    text("API Key").size(14).style(|_| text::Style{color: Some(Color::from_rgb(0.8,0.8,0.8)), ..Default::default()}),
                    text_input("Enter your Google API Key...", &self.advanced_settings.youtube_api_key)
                        .on_input(Message::YouTubeApiKeyChanged)
                        .padding(12)
                        .secure(true) 
                        .style(rounded_text_input_style),
                        
                    vertical_space().height(10),
                    
                    container(
                        row![
                            text("ℹ️").size(14),
                            column![
                                text("Need an API Key?").size(14).font(iced::Font{weight: iced::font::Weight::Bold, ..Default::default()}),
                                text("Go to console.cloud.google.com -> Create Project -> Enable YouTube Data API v3 -> Create Credentials -> API Key").size(12).style(|_| text::Style{color: Some(Color::from_rgb(0.7,0.7,0.7)), ..Default::default()})
                            ].spacing(4)
                        ].spacing(10).align_y(iced::alignment::Vertical::Top)
                    )
                    .padding(12)
                    .width(Length::Fill)
                    .style(|_| container::Style {
                        background: Some(iced::Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.05))),
                        border: iced::border::Border {
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.1),
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..Default::default()
                    })
                ]
            )
            .padding(25)
            .width(Length::Fill)
            .style(move |_| card_style(self.theme)),
            
            vertical_space().height(20),
            
            // Download Options
            container(
                column![
                    row![
                        text("⬇️").size(20),
                        text("Download Preferences").size(18).font(iced::Font{weight: iced::font::Weight::Bold, ..Default::default()})
                    ].spacing(10).align_y(iced::alignment::Vertical::Center),
                    
                    vertical_space().height(20),
                    
                    row![
                         // Checkbox column 1
                         column![
                             toggler(self.advanced_settings.embed_subs).label("📝 Embed Subtitles")
                                .on_toggle(Message::ToggleEmbedSubs),
                             vertical_space().height(10),
                             toggler(self.advanced_settings.embed_thumbnail).label("🖼️ Embed Thumbnail")
                                .on_toggle(Message::ToggleEmbedThumbnail),
                         ].spacing(5).width(Length::FillPortion(1)),
                         
                         // Checkbox column 2
                         column![
                             toggler(self.advanced_settings.restrict_filenames).label("🔤 ASCII Filenames Only")
                                .on_toggle(Message::ToggleRestrictFilenames),
                         ].spacing(5).width(Length::FillPortion(1))
                    ].spacing(20)
                ]
            )
            .padding(25)
            .width(Length::Fill)
            .style(move |_| card_style(self.theme)),
            
            vertical_space().height(20),
            
            // Network
            container(
                column![
                     row![
                        text("🌐").size(20),
                        text("Network & Cookies").size(18).font(iced::Font{weight: iced::font::Weight::Bold, ..Default::default()})
                    ].spacing(10).align_y(iced::alignment::Vertical::Center),
                    
                    vertical_space().height(20),
                    
                    text("Browser Cookies Source").size(14).style(|_| text::Style{color: Some(Color::from_rgb(0.8,0.8,0.8)), ..Default::default()}),
                    row![
                        pick_list(Browser::ALL, self.advanced_settings.cookies_browser, Message::BrowserSelected)
                            .padding(10)
                            .width(Length::Fill)
                            .style(pick_list::default),
                        
                        if self.advanced_settings.cookies_browser.is_some() {
                             let b: Element<'_, Message> = button(text("✖").size(14))
                                .on_press(Message::ClearBrowserCookies)
                                .padding(10)
                                .style(glass_danger_style)
                                .into();
                             b
                        } else {
                             horizontal_space().width(Length::Fixed(0.0)).into()
                        }
                    ].spacing(10).align_y(iced::alignment::Vertical::Center),
                    text("Select the browser where you are logged into YouTube (Premium/Age-restricted access)").size(12).style(|_| text::Style{color: Some(Color::from_rgb(0.5,0.5,0.5)), ..Default::default()}),
                        
                    vertical_space().height(15),
                    
                    text("Proxy URL").size(14).style(|_| text::Style{color: Some(Color::from_rgb(0.8,0.8,0.8)), ..Default::default()}),
                    text_input("http://user:pass@host:port", &self.advanced_settings.proxy_url)
                        .on_input(Message::ProxyChanged)
                        .padding(10)
                        .width(Length::Fill)
                        .style(rounded_text_input_style),
                ]
            )
            .padding(25)
            .width(Length::Fill)
            .style(move |_| card_style(self.theme)),
            
            vertical_space().height(40),
        ]
        .padding(40)
        .max_width(800)
        .width(Length::Fill);
        
        scrollable(
            container(content)
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
        )
        .into()
    }
}

impl OnyxApp {
    fn view_video_player(&self) -> Element<'_, Message> {
        // Title bar
        let title_text = text(&self.video_player_title)
            .size(18);
        
        let close_button = button(text("Close"))
            .padding(8)
            .style(glass_danger_style)
            .on_press(Message::CloseVideoPlayer);
        
        let title_bar = row![
            title_text,
            horizontal_space(),
            close_button
        ]
        .spacing(10)
        .padding(15)
        .align_y(iced::alignment::Vertical::Center);
        
        // Video player area
        let video_content: Element<'_, Message> = if let Some(video) = &self.video {
            let video_view = iced_video_player::VideoPlayer::new(video);
            let base_content = container(video_view)
                .width(Length::Fill)
                .height(Length::Fixed(400.0))
                .style(|_| container::Style::default());
            
            if self.video_crop_mode {
                stack![
                    base_content,
                    canvas(CropOverlay { 
                        selection: self.video_crop_selection.clone(), 
                        drag_start: self.crop_drag_start 
                    })
                    .width(Length::Fill)
                    .height(Length::Fill)
                ]
                .into()
            } else {
                base_content.into()
            }
        } else {
            container(text("No video loaded").size(20))
                .width(Length::Fill)
                .height(Length::Fixed(400.0))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style::default())
                .into()
        };



        let video_placeholder = container(video_content)
            .width(900)
            .height(500)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(|_| container::Style::default());
        
        // Apple-style timeline trimmer
        let timeline = self.view_timeline_trimmer();
        
        // Selection info
        let selection_duration = self.trimmer_end - self.trimmer_start;
        let selection_info = text(format!(
            "Selection: {} - {} (Duration: {})",
            format_time(self.trimmer_start),
            format_time(self.trimmer_end),
            format_time(selection_duration)
        ))
        .size(14)
        ;
        // Action buttons
        let crop_button = button(text(if self.video_crop_mode { "Done Cropping" } else { "Crop Video" }))
            .padding(15)
            .style(move |_, s| if self.video_crop_mode { glass_primary_style(s, self.theme) } else { glass_secondary_style(s, self.theme) })
            .on_press(Message::ToggleCropMode);

        let add_button = if self.editing_queue_item.is_some() {
             button(text("Save Changes").size(16))
                 .padding(15)
                 .style(move |_, s| glass_primary_style(s, self.theme))
                 .on_press(Message::UpdateQueueItem)
        } else {
             button(text("Add Trimmed Selection to Queue").size(16))
                 .padding(15)
                 .style(move |_, s| glass_primary_style(s, self.theme))
                 .on_press(Message::AddTrimmedToQueue)
        };
        
        let cancel_button = button(
            text("Cancel")
        )
        .padding(10)
        .style(move |_, s| glass_secondary_style(s, self.theme))
        .on_press(Message::CloseVideoPlayer);
        
        // Player content
        let player_content = column![
            title_bar,
            video_placeholder,
            vertical_space().height(20),
            timeline,
            vertical_space().height(15),
            selection_info,
            vertical_space().height(20),
            row![
                cancel_button,
                horizontal_space().width(20),
                crop_button,
                horizontal_space().width(20),
                add_button,
            ]
            .align_y(iced::alignment::Vertical::Center),
        ]
        .spacing(0)
        .padding(20)
        .align_x(iced::alignment::Horizontal::Center);
        
        player_content.into()
    }
    
    fn view_timeline_trimmer(&self) -> Element<'_, Message> {
        let timeline_width = 800.0;
        let height = 60.0;
        
        let trimmer_program = TimelineTrimmer {
            duration: self.video_player_duration,
            start: self.trimmer_start,
            end: self.trimmer_end,
            dragging: self.dragging_handle,
            hover: self.hover_handle,
        };
        
        column![
            text(""),
            vertical_space().height(10),
            
            // Custom Canvas-based Trimmer
            container(
                canvas(trimmer_program)
                    .width(Length::Fixed(timeline_width))
                    .height(Length::Fixed(height))
            )
            .padding(2)
            .style(|_| container::Style::default()),
            
            vertical_space().height(5),
            
            // Time Labels
            row![
                text(format!("Start: {}", format_time(self.trimmer_start))).size(12),
                horizontal_space(),
                text(format!("End: {}", format_time(self.trimmer_end))).size(12),
            ]
            .width(Length::Fixed(timeline_width)),
            
            vertical_space().height(5),
            text("Drag yellow handles to adjust trim range")
                .size(11)
                
        ]
        .spacing(0)
        .align_x(iced::alignment::Horizontal::Center)
        .into()
    }
    
    fn view_dependency_error(&self, _error: &str, downloading: bool, progress: f32) -> Element<'_, Message> {
        let logo = canvas(AnimatedLogo { tick: self.start_time.elapsed().as_secs_f32() })
            .width(Length::Fixed(80.0))
            .height(Length::Fixed(80.0));
        
        let title = text("ONYX")
            .size(48)
            .font(iced::Font { weight: iced::font::Weight::Bold, ..Default::default() });
        
        let error_icon = canvas(ErrorIcon)
            .width(Length::Fixed(120.0))
            .height(Length::Fixed(120.0));
        
        let error_msg = text(format!("Dependencies Required"))
            .size(28);
        
        let description = text("Onyx needs yt-dlp and ffmpeg to download videos")
            .size(16);
        
        let action_content: Element<'_, Message> = if downloading {
            // Estimate time remaining (rough estimate: 60 seconds total)
            let estimated_total = 60.0;
            let elapsed = self.start_time.elapsed().as_secs_f32();
            let remaining = (estimated_total - elapsed).max(0.0);
            let minutes = (remaining / 60.0).floor() as u32;
            let seconds = (remaining % 60.0) as u32;
            
            column![
                text(""),
                vertical_space().height(20),
                // Running character animation
                container(
                    canvas(RunningCharacter { 
                        progress: progress, 
                        tick: self.start_time.elapsed().as_secs_f32() 
                    })
                    .width(Length::Fill)
                    .height(Length::Fixed(80.0))
                )
                .width(Length::Fixed(500.0))
                .padding(10)
                .style(move |_| card_style(self.theme)),
                vertical_space().height(20),
                // Progress bar
                container(
                    canvas(ProgressBar { progress: progress * 100.0, tick: self.start_time.elapsed().as_secs_f32() })
                        .width(Length::Fill)
                        .height(Length::Fixed(14.0))
                )
                .width(Length::Fixed(500.0))
                .padding(10)
                .style(move |_| card_style(self.theme)),
                vertical_space().height(15),
                // Countdown timer
                row![
                    text(format!("⏱ Estimated time: {}:{:02}", minutes, seconds))
                        .size(14),
                    horizontal_space().width(30),
                    text(format!("{:.0}%", progress * 100.0))
                        .size(14)
                        ,
                ]
                .align_y(iced::alignment::Vertical::Center),
                vertical_space().height(10),
                text(""),
            ]
            .align_x(iced::alignment::Horizontal::Center)
            .into()
        } else {
            button(
                text("Download Dependencies")
                    .size(18)
                    
            )
            .padding(20)
            .width(Length::Fixed(300.0))
            .style(move |_, s| glass_primary_style(s, self.theme))
            .on_press(Message::DownloadDependencies)
            .into()
        };
        
        let content = column![
            row![logo, horizontal_space().width(20), title].align_y(iced::alignment::Vertical::Center),
            vertical_space().height(60),
            error_icon,
            vertical_space().height(30),
            error_msg,
            vertical_space().height(15),
            description,
            vertical_space().height(40),
            action_content
        ]
        .align_x(iced::alignment::Horizontal::Center)
        .padding(60);
        
        container(
            container(content)
                .width(Length::Fixed(700.0))
                .padding(50)
                .style(move |_| card_style(self.theme))
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_| main_background_style(self.theme))
        .into()
    }
    
    fn view_quick_download(&self) -> Element<'_, Message> {
        // Thumbnail
        let thumbnail: Element<'_, Message> = if let Some(handle) = &self.thumbnail {
            container(image(handle.clone()).width(Length::Fixed(320.0)).height(Length::Fixed(180.0)))
                .style(move |_| card_style(self.theme)).padding(5).into()
        } else {
            vertical_space().height(Length::Shrink).into()
        };

        let input = text_input("Paste YouTube Link...", &self.url)
            .on_input(Message::UrlChanged)
            .padding(15)
            .size(16)
            .style(rounded_text_input_style);
        
        let format_selector = column![
            text("Options").size(14).font(iced::Font{weight: iced::font::Weight::Bold, ..Default::default()}),
            row(
                DownloadFormat::all().iter().map(|fmt| {
                    let is_selected = self.format == *fmt;
                    let label = match fmt {
                        DownloadFormat::VideoBest => "Best Video",
                        DownloadFormat::Video1080p => "1080p",
                        DownloadFormat::Video720p => "720p",
                        DownloadFormat::AudioBest => "Best Audio",
                        DownloadFormat::AudioMp3 => "MP3",
                    };
                    
                    button(text(label).size(12))
                        .padding([6, 12])
                        .style(move |_, s| if is_selected { glass_primary_style(s, self.theme) } else { glass_secondary_style(s, self.theme) })
                        .on_press(Message::FormatSelected(*fmt))
                        .into()
                }).collect::<Vec<_>>()
            ).spacing(8)
        ].spacing(10).align_x(iced::alignment::Horizontal::Center);

        let path_selector = row![
             button(row![text("📂").size(16), text("Browse").size(14)].spacing(8).align_y(iced::alignment::Vertical::Center))
                 .on_press(Message::BrowseFolder)
                 .padding([10, 15])
                 .style(move |_, s| glass_secondary_style(s, self.theme)),
             
             container(
                 row![
                      text("💾").size(14),
                      text(shorten_path(&self.download_folder)).size(14)
                 ].spacing(10).align_y(iced::alignment::Vertical::Center)
             )
             .padding([8, 12])
             .style(|_| container::Style {
                 background: Some(iced::Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.05))),
                 border: iced::border::Border {
                     color: Color::from_rgba(1.0, 1.0, 1.0, 0.1),
                     width: 1.0,
                     radius: 6.0.into()
                 },
                 ..Default::default()
             })
        ]
        .spacing(10)
        .align_y(iced::alignment::Vertical::Center);

        let basic_controls = column![
            format_selector,
            path_selector
        ]
        .spacing(15)
        .align_x(iced::alignment::Horizontal::Center);




        let is_downloading = matches!(self.state, AppState::Downloading { .. });
        let download_btn = button(
             container(
                 if is_downloading {
                     row![text("⏳").size(24), text("DOWNLOADING...").size(16)].spacing(10).align_y(iced::alignment::Vertical::Center)
                 } else {
                     row![text("🚀").size(24), text("START DOWNLOAD").size(16)].spacing(10).align_y(iced::alignment::Vertical::Center)
                 }
             )
             .width(Length::Fill)
             .align_x(iced::alignment::Horizontal::Center)
        )
        .padding(18)
        .width(Length::Fill)
        .style(move |_, s| glass_primary_style(s, self.theme));
        
        let download_btn = if !is_downloading && self.dependencies_ok {
            download_btn.on_press(Message::DownloadPressed)
        } else {
            download_btn
        };

        let status_area = match &self.state {
            AppState::Idle | AppState::CheckingDependencies => column![vertical_space().height(40)],
            AppState::Downloading { progress, status_text: _ } => {
                column![
                    // Status text with parsed info
                    text(""),
                    vertical_space().height(15),
                    // Stopwatch and running character row
                    row![
                        // Stopwatch timer
                        container(
                            canvas(Stopwatch { 
                                elapsed: self.start_time.elapsed().as_secs_f32() 
                            })
                            .width(Length::Fixed(60.0))
                            .height(Length::Fixed(60.0))
                        )
                        .padding(10)
                        .style(move |_| card_style(self.theme)),
                        horizontal_space().width(15),
                        // Running character animation
                        container(
                            canvas(RunningCharacter { 
                                progress: *progress / 100.0, 
                                tick: self.start_time.elapsed().as_secs_f32() 
                            })
                            .width(Length::Fill)
                            .height(Length::Fixed(80.0))
                        )
                        .width(Length::Fill)
                        .padding(10)
                        .style(move |_| card_style(self.theme)),
                    ]
                    .align_y(iced::alignment::Vertical::Center),
                    vertical_space().height(10),
                    canvas(ProgressBar { progress: *progress, tick: self.start_time.elapsed().as_secs_f32() })
                        .width(Length::Fill)
                        .height(Length::Fixed(14.0))
                ]
                .spacing(5)
            }
            AppState::Finished(result) => match result {
                Ok(_) => column![
                    text(""),
                    vertical_space().height(10),
                    canvas(ProgressBar { progress: 100.0, tick: self.start_time.elapsed().as_secs_f32() })
                        .width(Length::Fill)
                        .height(Length::Fixed(12.0))
                ],
                Err(_e) => column![
                    text(""),
                    text(""),
                ]
                .spacing(5)
            },
            _ => column![vertical_space().height(40)],
        };

        let content = scrollable(
            column![
                thumbnail,
                vertical_space().height(20),
                input,
                vertical_space().height(20),
                basic_controls,
                vertical_space().height(20),

                vertical_space().height(20),
                // Time range section
                if let Some(duration) = self.video_duration {
                    let is_active = self.quick_time_range.is_some();
                    
                    let header = row![
                         text("✂️ Download Section").size(16).font(iced::Font{weight: iced::font::Weight::Bold, ..Default::default()}),
                         horizontal_space(),
                         checkbox(is_active).on_toggle(Message::ToggleQuickTimeRange)
                    ].align_y(iced::alignment::Vertical::Center);
                    
                    let time_range_controls: Element<'_, Message> = if let Some(range) = &self.quick_time_range {
                        column![
                                row![
                                    text("Start").width(Length::Fixed(40.0)).size(12),
                                    slider(0.0..=range.end_seconds, range.start_seconds, Message::UpdateQuickTimeRangeStart)
                                        .width(Length::Fill),
                                    text(format_duration(range.start_seconds)).size(12).width(Length::Fixed(60.0)).align_x(iced::alignment::Horizontal::Right),
                                ]
                                .spacing(10)
                                .align_y(iced::alignment::Vertical::Center),
                                
                                row![
                                    text("End").width(Length::Fixed(40.0)).size(12),
                                    slider(range.start_seconds..=duration, range.end_seconds, Message::UpdateQuickTimeRangeEnd)
                                        .width(Length::Fill),
                                    text(format_duration(range.end_seconds)).size(12).width(Length::Fixed(60.0)).align_x(iced::alignment::Horizontal::Right),
                                ]
                                .spacing(10)
                                .align_y(iced::alignment::Vertical::Center),
                        ]
                        .spacing(15)
                        .padding(10)
                        .into()
                    } else {
                        horizontal_space().width(0).into()
                    };
                    
                    container(
                        column![
                            header,
                            if is_active { vertical_space().height(10) } else { vertical_space().height(0) },
                            time_range_controls
                        ]
                    )
                    .width(Length::Fill)
                    .padding(15)
                    .style(move |_| card_style(self.theme))
                    .into()
                } else {
                    Element::from(column![])
                },
                vertical_space().height(20),
                download_btn,
                vertical_space().height(30),
                status_area
            ]
            .padding(40)
            .align_x(iced::alignment::Horizontal::Center)
            .spacing(10));
        
        container(content)
            .width(Length::Fill)
            .max_width(800)
            .style(move |_| card_style(self.theme))
            .into()
    }
    
    fn view_batch_queue(&self) -> Element<'_, Message> {
        // URL input section
        let url_input_row = row![
            text_input("Paste YouTube URL...", &self.queue_url_input)
                .on_input(Message::QueueUrlInputChanged)
                .padding(12)
                .size(16)
                .width(Length::Fill)
                .style(rounded_text_input_style),
            button(row![text("➕").size(16), text("Add to Queue").size(14)].spacing(8).align_y(iced::alignment::Vertical::Center))
                .on_press(Message::AddToQueue)
                .padding([10, 15])
                .style(move |_, s| glass_primary_style(s, self.theme))
        ]
        .spacing(10)
        .align_y(iced::alignment::Vertical::Center);
        
        let folder_row = row![
             button(row![text("📂").size(16), text("Browse").size(14)].spacing(8).align_y(iced::alignment::Vertical::Center))
                 .on_press(Message::BrowseFolder)
                 .padding([10, 15])
                 .style(move |_, s| glass_secondary_style(s, self.theme)),
             
             container(
                 row![
                      text("💾").size(14),
                      text(shorten_path(&self.download_folder)).size(14)
                 ].spacing(10).align_y(iced::alignment::Vertical::Center)
             )
             .padding([8, 12])
             .style(|_| container::Style {
                 background: Some(iced::Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.05))),
                 border: iced::border::Border {
                     color: Color::from_rgba(1.0, 1.0, 1.0, 0.1),
                     width: 1.0,
                     radius: 6.0.into()
                 },
                 ..Default::default()
             })
        ]
        .spacing(10)
        .align_y(iced::alignment::Vertical::Center);
        
        // Queue list
        // Queue listheader
        let queue_header = row![
            text("Queue").size(18).font(iced::Font{weight: iced::font::Weight::Bold, ..Default::default()}),
            text(format!("({} items)", self.download_queue.len())).size(14).style(|_| text::Style{color: Some(Color::from_rgb(0.7,0.7,0.7)), ..Default::default()}),
            horizontal_space().width(Length::Fill),
            if self.active_downloads > 0 {
                row![
                    text("⏳").size(16),
                    text(format!("Downloading {}...", self.active_downloads)).size(14)
                ].spacing(5).align_y(iced::alignment::Vertical::Center)
            } else {
                row![].into()
            }
        ]
        .spacing(10)
        .align_y(iced::alignment::Vertical::Center);
        
        let queue_items: Element<'_, Message> = if self.download_queue.is_empty() {
             container(
                 column![
                     text("📭").size(32),
                     text("Your queue is empty").size(16).style(|_| text::Style{color: Some(Color::from_rgb(0.8,0.8,0.8)), ..Default::default()}),
                     text("Add videos to start downloading").size(13).style(|_| text::Style{color: Some(Color::from_rgb(0.5,0.5,0.5)), ..Default::default()})
                 ]
                 .spacing(8)
                 .align_x(iced::alignment::Horizontal::Center)
             )
             .width(Length::Fill)
             .height(Length::Fixed(180.0))
             .align_x(iced::alignment::Horizontal::Center)
             .align_y(iced::alignment::Vertical::Center)
             .style(|_| container::Style {
                 background: Some(iced::Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.02))),
                 border: iced::border::Border {
                     color: Color::from_rgba(1.0, 1.0, 1.0, 0.05),
                     width: 1.0,
                     radius: 12.0.into(),
                 },
                 ..Default::default()
             })
             .into()
        } else {
            let items = self.download_queue.iter().enumerate().fold(
                column![].spacing(10),
                |col, (idx, item)| {
                    col.push(self.view_queue_item(item, idx))
                }
            );
            scrollable(items)
                .height(Length::Fill)
                .into()
        };
        
        // Download all button
        let ready_count = self.download_queue.iter()
            .filter(|item| matches!(item.status, QueueStatus::Ready))
            .count();
        
        let download_all_btn = if ready_count > 0 && self.active_downloads == 0 {
            button(
                container(
                    row![text("🚀").size(24), text(format!("DOWNLOAD ALL ({} ready)", ready_count)).size(16)].spacing(10).align_y(iced::alignment::Vertical::Center)
                ).width(Length::Fill).align_x(iced::alignment::Horizontal::Center)
            )
            .padding(18)
            .width(Length::Fill)
            .style(move |_, s| glass_primary_style(s, self.theme))
            .on_press(Message::StartBatchDownload)
        } else if self.active_downloads > 0 {
            button(
                container(
                    row![text("⏳").size(24), text(format!("DOWNLOADING... ({}/{})", self.active_downloads, self.download_queue.len())).size(16)].spacing(10).align_y(iced::alignment::Vertical::Center)
                ).width(Length::Fill).align_x(iced::alignment::Horizontal::Center)
            )
            .padding(18)
            .width(Length::Fill)
            .style(move |_, s| glass_secondary_style(s, self.theme))
        } else {
            button(
                container(
                    row![text("🚫").size(24), text("NO ITEMS READY").size(16)].spacing(10).align_y(iced::alignment::Vertical::Center)
                ).width(Length::Fill).align_x(iced::alignment::Horizontal::Center)
            )
            .padding(18)
            .width(Length::Fill)
            .style(move |_, s| glass_secondary_style(s, self.theme))
        };
        
        let content = column![
            url_input_row,
            vertical_space().height(10),
            folder_row,
            vertical_space().height(20),
            queue_header,
            vertical_space().height(10),
            queue_items,
            vertical_space().height(20),
            download_all_btn
        ]
        .padding(20)
        .spacing(10);
        
        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| card_style(self.theme))
            .into()
    }
    
    fn view_queue_item<'a>(&'a self, item: &'a QueueItem, _index: usize) -> Element<'a, Message> {
        // Thumbnail
        let thumb = if let Some(handle) = &item.thumbnail {
            container(image(handle.clone()).width(Length::Fixed(120.0)).height(Length::Fixed(68.0)).content_fit(iced::ContentFit::Cover))
                .style(|_| container::Style {
                    border: iced::border::Border {
                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.1),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .padding(0)
        } else {
            container(text(""))
                .width(Length::Fixed(120.0))
                .height(Length::Fixed(68.0))
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_| card_style(self.theme))
        };
        
        // Title and duration
        let _title_text = item.title.clone().unwrap_or_else(|| "Fetching info...".to_string());
        let duration_text = if let Some(dur) = item.duration {
            format_duration(dur)
        } else {
            "--:--".to_string()
        };
        

        
        // Status indicator
        let status_widget: Element<'_, Message> = match &item.status {
            QueueStatus::Fetching => text("").into(),
            QueueStatus::Ready => text("").into(),
            QueueStatus::Downloading(progress) => {
                let tick = self.start_time.elapsed().as_secs_f32();
                row![
                    // Running Man Animation
                    container(
                         canvas(RunningCharacter { 
                             progress: *progress, 
                             tick 
                         })
                         .width(Length::Fixed(150.0))
                         .height(Length::Fixed(50.0))
                    )
                    .padding(5)
                    .style(move |_| card_style(self.theme)),
                    
                    horizontal_space().width(8),
                    
                    // Progress Text
                    column![
                        text(format!("{:.0}%", *progress * 100.0)).size(14).style(|_| iced::widget::text::Style { color: Some(Color::WHITE) }),
                        // Small stopwatch icon/anim below text
                        canvas(Stopwatch { elapsed: tick })
                            .width(Length::Fixed(24.0))
                            .height(Length::Fixed(24.0))
                    ]
                    .align_x(iced::alignment::Horizontal::Center)
                ]
                .align_y(iced::alignment::Vertical::Center)
                .into()
            }
            QueueStatus::Complete => text("").into(),
            QueueStatus::Failed(err) => text(format!("✗ {}", err)).size(12).into(),
        };
        
        // Media Type Controls (Small)
        let is_video = item.media_type == MediaType::Video;
        let media_picker = row![
            button(text("Video").size(10))
                .padding([4, 8])
                .style(move |_, s| if is_video { glass_primary_style(s, self.theme) } else { glass_secondary_style(s, self.theme) })
                .on_press(Message::UpdateQueueItemMediaType(item.id, MediaType::Video)),
            button(text("Audio").size(10))
                .padding([4, 8])
                .style(move |_, s| if !is_video { glass_primary_style(s, self.theme) } else { glass_secondary_style(s, self.theme) })
                .on_press(Message::UpdateQueueItemMediaType(item.id, MediaType::Audio)),
        ].spacing(5);
        
        // Format Controls (Small)
        let formats = OutputFormat::for_media_type(item.media_type);
        let format_picker = row(
            formats.iter().map(|fmt| {
                let is_selected = item.output_format == *fmt;
                button(text(fmt.to_string()).size(10))
                    .padding([4, 8])
                    .style(move |_, s| if is_selected { glass_primary_style(s, self.theme) } else { glass_secondary_style(s, self.theme) })
                    .on_press(Message::UpdateQueueItemFormat(item.id, *fmt))
                    .into()
            }).collect::<Vec<_>>()
        ).spacing(5);
        

        

        
        let remove_btn = button(text("×").size(20).line_height(1.0))
            .padding([0, 8])
            .style(glass_danger_style)
            .on_press(Message::RemoveQueueItem(item.id));

        // Sidebar
        let left_sidebar = column![
            thumb,
            vertical_space().height(10),
            text("TYPE").size(10).style(|_| text::Style{color:Some(Color::from_rgb(0.5,0.5,0.5)), ..Default::default()}),
            media_picker,
            vertical_space().height(8),
            text("FORMAT").size(10).style(|_| text::Style{color:Some(Color::from_rgb(0.5,0.5,0.5)), ..Default::default()}),
            format_picker
        ]
        .align_x(iced::alignment::Horizontal::Center)
        .width(Length::Fixed(160.0));

        // Trim/Section Button
        let trim_label = if let Some(range) = &item.time_range {
            format!("Trim: {}-{}", format_duration(range.start_seconds), format_duration(range.end_seconds))
        } else {
             "Trim: Full Video".to_string()
        };
        
        let is_trimmed = item.time_range.is_some();
        let trim_btn = button(
             row![
                 text("✂️").size(14),
                 text(trim_label).size(12)
             ].spacing(8).align_y(iced::alignment::Vertical::Center)
        )
        .padding([8, 12])
        .style(move |_, s| if is_trimmed { glass_primary_style(s, self.theme) } else { glass_secondary_style(s, self.theme) })
        .on_press(Message::EditQueueItem(item.id));

        // Right Content
        let main_content = column![
            // Header
            row![
                text(item.title.as_deref().unwrap_or("Fetching...")).size(16).font(iced::Font { weight: iced::font::Weight::Bold, ..Default::default() }).width(Length::Fill),
                horizontal_space().width(10),
                remove_btn
            ].align_y(iced::alignment::Vertical::Center),
            
            vertical_space().height(4),
            
            // Meta Row
             row![
                text(format!("{}", duration_text)).size(12).style(|_| text::Style{color:Some(Color::from_rgb(0.6,0.6,0.6)), ..Default::default()}),
                horizontal_space().width(20),
                status_widget
            ].align_y(iced::alignment::Vertical::Center),
            
            vertical_space().height(15), 
            
            // Actions Row
            row![
                trim_btn,
                horizontal_space().width(Length::Fill)
            ].align_y(iced::alignment::Vertical::Center)
        ]
        .spacing(5);
        
        container(
            row![
                left_sidebar,
                horizontal_space().width(25),
                main_content
            ]
        )
        .padding(20)
        .style(move |_| card_style(self.theme))
        .width(Length::Fill)
        .into()
    }
}

// Continue in next message...
// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

// Spacing helpers for iced 0.14
fn horizontal_space() -> Space {
    Space::new().width(Length::Fill)
}

fn vertical_space() -> Space {
    Space::new().height(Length::Fill)
}

// Utility: Shorten path for display
fn shorten_path(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| format!(".../{}", s))
        .unwrap_or_else(|| path.display().to_string())
}

// Utility: Format duration in seconds to MM:SS or HH:MM:SS
fn format_duration(seconds: f32) -> String {
    let total_secs = seconds as u32;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}", minutes, secs)
    }
}

// Utility: Format time range for yt-dlp
// Format: *HH:MM:SS-HH:MM:SS (MUST always include hours for yt-dlp)
fn format_time_range_for_ytdlp(range: &TimeRange) -> String {
    let format_for_ytdlp = |seconds: f32| -> String {
        let total_secs = seconds as u32;
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        
        // Use simplest format: seconds only if < 60s, MM:SS if < 1hr, HH:MM:SS otherwise
        if total_secs < 60 {
            format!("{}", total_secs)
        } else if hours == 0 {
            format!("{:02}:{:02}", minutes, secs)
        } else {
            format!("{:02}:{:02}:{:02}", hours, minutes, secs)
        }
    };
    
    format!("*{}-{}", 
        format_for_ytdlp(range.start_seconds),
        format_for_ytdlp(range.end_seconds)
    )
}

// Parse duration string (e.g., "3:45", "1:23:45") to seconds
fn parse_duration_to_seconds(duration_str: &str) -> f32 {
    let parts: Vec<&str> = duration_str.split(':').collect();
    match parts.len() {
        1 => parts[0].parse().unwrap_or(0.0), // seconds only
        2 => {
            // MM:SS
            let mins: f32 = parts[0].parse().unwrap_or(0.0);
            let secs: f32 = parts[1].parse().unwrap_or(0.0);
            mins * 60.0 + secs
        }
        3 => {
            // HH:MM:SS
            let hours: f32 = parts[0].parse().unwrap_or(0.0);
            let mins: f32 = parts[1].parse().unwrap_or(0.0);
            let secs: f32 = parts[2].parse().unwrap_or(0.0);
            hours * 3600.0 + mins * 60.0 + secs
        }
        _ => 0.0,
    }
}

// Format seconds to time string (MM:SS or HH:MM:SS)
fn format_time(seconds: f32) -> String {
    let total_secs = seconds as u32;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;
    
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}", minutes, secs)
    }
}

// ============================================================================
// YOUTUBE BROWSER FUNCTIONS (YouTube Data API v3)
// ============================================================================

// Search YouTube videos using YouTube Data API v3
async fn search_youtube(query: String, api_key: String) -> Vec<YouTubeVideo> {
    if api_key.is_empty() {
        return Vec::new();
    }
    
    let url = format!(
        "https://www.googleapis.com/youtube/v3/search?part=snippet&q={}&type=video&maxResults=10&key={}",
        urlencoding::encode(&query),
        api_key
    );
    match reqwest::get(&url).await {
        Ok(response) => {
            if let Ok(text) = response.text().await {
                parse_youtube_api_search(&text)
            } else {
                Vec::new()
            }
        }
        Err(_) => Vec::new(),
    }
}

// Load trending videos using YouTube Data API v3
async fn load_trending_videos(api_key: String) -> Vec<YouTubeVideo> {
    if api_key.is_empty() {
        return Vec::new();
    }
    
    let url = format!(
        "https://www.googleapis.com/youtube/v3/videos?part=snippet,statistics&chart=mostPopular&maxResults=20&regionCode=US&key={}",
        api_key
    );
    match reqwest::get(&url).await {
        Ok(response) => {
            if let Ok(text) = response.text().await {
                parse_youtube_api_videos(&text)
            } else {
                Vec::new()
            }
        }
        Err(_) => Vec::new(),
    }
}

// Parse YouTube API search response
fn parse_youtube_api_search(data: &str) -> Vec<YouTubeVideo> {
    let mut videos = Vec::new();
    
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
        if let Some(items) = json["items"].as_array() {
            for item in items {
                let id = item["id"]["videoId"].as_str().unwrap_or("").to_string();
                let snippet = &item["snippet"];
                let title = snippet["title"].as_str().unwrap_or("Unknown").to_string();
                let channel = snippet["channelTitle"].as_str().unwrap_or("Unknown").to_string();
                let thumbnail_url = snippet["thumbnails"]["medium"]["url"]
                    .as_str()
                    .or(snippet["thumbnails"]["default"]["url"].as_str())
                    .unwrap_or("")
                    .to_string();
                
                if !id.is_empty() {
                    videos.push(YouTubeVideo {
                        id: id.clone(),
                        title,
                        channel,
                        thumbnail_url,
                        duration: "N/A".to_string(), // Search API doesn't include duration
                        views: "N/A".to_string(),     // Search API doesn't include views
                        url: format!("https://www.youtube.com/watch?v={}", id),
                    });
                }
            }
        }
    }
    
    videos
}

// Parse YouTube API videos response (for trending)
fn parse_youtube_api_videos(data: &str) -> Vec<YouTubeVideo> {
    let mut videos = Vec::new();
    
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
        if let Some(items) = json["items"].as_array() {
            for item in items {
                let id = item["id"].as_str().unwrap_or("").to_string();
                let snippet = &item["snippet"];
                let statistics = &item["statistics"];
                
                let title = snippet["title"].as_str().unwrap_or("Unknown").to_string();
                let channel = snippet["channelTitle"].as_str().unwrap_or("Unknown").to_string();
                let thumbnail_url = snippet["thumbnails"]["medium"]["url"]
                    .as_str()
                    .or(snippet["thumbnails"]["default"]["url"].as_str())
                    .unwrap_or("")
                    .to_string();
                
                let view_count = statistics["viewCount"]
                    .as_str()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                let views = format_view_count(view_count);
                
                if !id.is_empty() {
                    videos.push(YouTubeVideo {
                        id: id.clone(),
                        title,
                        channel,
                        thumbnail_url,
                        duration: "N/A".to_string(), // Videos API doesn't include duration in snippet
                        views,
                        url: format!("https://www.youtube.com/watch?v={}", id),
                    });
                }
            }
        }
    }
    
    videos
}

// Parse yt-dlp JSON output (DEPRECATED - keeping for reference)
fn parse_youtube_json(data: &[u8]) -> Vec<YouTubeVideo> {
    let text = String::from_utf8_lossy(data);
    let mut videos = Vec::new();
    
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            let id = json["id"].as_str().unwrap_or("").to_string();
            let title = json["title"].as_str().unwrap_or("Unknown").to_string();
            let channel = json["uploader"].as_str().or(json["channel"].as_str()).unwrap_or("Unknown").to_string();
            let thumbnail_url = json["thumbnail"].as_str().unwrap_or("").to_string();
            let duration_secs = json["duration"].as_f64().unwrap_or(0.0) as u32;
            let duration = format_duration_string(duration_secs);
            let view_count = json["view_count"].as_u64().unwrap_or(0);
            let views = format_view_count(view_count);
            let url = json["webpage_url"].as_str().or(json["url"].as_str()).unwrap_or("").to_string();
            
            if !id.is_empty() && !url.is_empty() {
                videos.push(YouTubeVideo {
                    id,
                    title,
                    channel,
                    thumbnail_url,
                    duration,
                    views,
                    url,
                });
            }
        }
    }
    
    videos
}

// Load YouTube thumbnail
async fn load_youtube_thumbnail(url: String) -> image::Handle {
    match reqwest::get(&url).await {
        Ok(response) => {
            match response.bytes().await {
                Ok(bytes) => image::Handle::from_bytes(bytes.to_vec()),
                Err(_) => image::Handle::from_bytes(Vec::new()),
            }
        }
        Err(_) => image::Handle::from_bytes(Vec::new()),
    }
}

// Format duration as MM:SS or HH:MM:SS
fn format_duration_string(seconds: u32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}", minutes, secs)
    }
}

// Format view count (e.g., 1.2M, 500K)
fn format_view_count(views: u64) -> String {
    if views >= 1_000_000 {
        format!("{:.1}M", views as f64 / 1_000_000.0)
    } else if views >= 1_000 {
        format!("{:.1}K", views as f64 / 1_000.0)
    } else {
        views.to_string()
    }
}

// Parse duration string (MM:SS or HH:MM:SS) to seconds
fn parse_duration(duration_str: &str) -> Option<f32> {
    let parts: Vec<&str> = duration_str.split(':').collect();
    match parts.len() {
        2 => {
            // MM:SS
            let minutes: f32 = parts[0].parse().ok()?;
            let seconds: f32 = parts[1].parse().ok()?;
            Some(minutes * 60.0 + seconds)
        }
        3 => {
            // HH:MM:SS
            let hours: f32 = parts[0].parse().ok()?;
            let minutes: f32 = parts[1].parse().ok()?;
            let seconds: f32 = parts[2].parse().ok()?;
            Some(hours * 3600.0 + minutes * 60.0 + seconds)
        }
        _ => None,
    }
}

// Resolve direct stream URL using yt-dlp
async fn resolve_stream_url(url: String, settings: AdvancedSettings) -> Result<(String, f32), String> {
    let bin_dir = get_bin_dir();
    let local_yt = bin_dir.join(if cfg!(target_os = "windows") { "yt-dlp.exe" } else { "yt-dlp" });
    let yt_binary = if local_yt.exists() { 
        local_yt.to_string_lossy().to_string() 
    } else { 
        if cfg!(target_os = "windows") { "yt-dlp.exe".to_string() } else { "yt-dlp".to_string() }
    };
    
    let mut cmd = tokio::process::Command::new(yt_binary);
    
    // Use --print to get duration AND url in defined order
    cmd.arg("--print").arg("duration")
       .arg("--print").arg("urls") 
       .arg("-f").arg("best[ext=mp4][protocol^=http]/best[protocol^=http]") // Force progressive HTTP stream (avoids HLS issues)
       .arg(&url);

    // Apply settings
    if !settings.proxy_url.is_empty() {
        cmd.arg("--proxy").arg(&settings.proxy_url);
    }
    if let Some(browser) = settings.cookies_browser {
        cmd.arg("--cookies-from-browser").arg(format!("{:?}", browser).to_lowercase());
    }
    // Note: restrictive filenames etc don't apply to stream resolution

    let output = cmd.output().await.map_err(|e| e.to_string())?;
    
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut lines = stdout.lines();
        
        let duration_str = lines.next().unwrap_or("0").trim();
        let stream_url = lines.next().unwrap_or("").trim().to_string();
        
        let duration = duration_str.parse::<f32>().unwrap_or(0.0);
        
        if stream_url.is_empty() {
            Err("Empty URL returned by yt-dlp".to_string())
        } else {
            Ok((stream_url, duration))
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(stderr.to_string())
    }
}

// Check if dependencies are installed
async fn check_dependencies() -> (bool, String) {
    let bin_dir = get_bin_dir();
    let local_yt = bin_dir.join(if cfg!(target_os = "windows") { "yt-dlp.exe" } else { "yt-dlp" });
    let local_ffmpeg = bin_dir.join(if cfg!(target_os = "windows") { "ffmpeg.exe" } else { "ffmpeg" }); // Usually not downloaded, but checked
    
    if local_yt.exists() && local_ffmpeg.exists() {
         return (true, "Using Local Binaries".to_string());
    }

    let yt_check = tokio::process::Command::new("yt-dlp").arg("--version").output().await;
    let ffmpeg_check = tokio::process::Command::new("ffmpeg").arg("-version").output().await;
    
    if yt_check.is_ok() && ffmpeg_check.is_ok() {
        if local_yt.exists() {
             return (true, "Using Local Binaries".to_string());
        }
        return (true, "System Binaries Found".to_string());
    }
    
    (false, "Dependencies Missing".to_string())
}

// Download dependencies
// Download dependencies
async fn download_dependencies_task() -> Result<(), String> {
    let bin_dir = get_bin_dir();
    if !bin_dir.exists() {
        tokio::fs::create_dir_all(&bin_dir).await.map_err(|e| e.to_string())?;
    }

    // Download yt-dlp
    let yt_url = if cfg!(target_os = "macos") {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos"
    } else if cfg!(target_os = "windows") {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    } else {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
    };

    // Only download if explicit download requested or missing
    // But this task is called on "Download Dependencies" button press, so always download.
    
    let response = reqwest::get(yt_url).await.map_err(|e| format!("Failed to connect to yt-dlp release: {}", e))?;
    let bytes = response.bytes().await.map_err(|e| format!("Failed to download yt-dlp: {}", e))?;
    
    let yt_path = bin_dir.join(if cfg!(target_os = "windows") { "yt-dlp.exe" } else { "yt-dlp" });
    let mut file = tokio::fs::File::create(&yt_path).await.map_err(|e| e.to_string())?;
    file.write_all(&bytes).await.map_err(|e| e.to_string())?;
    
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = file.metadata().await {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = file.set_permissions(perms).await;
        }
    }


    // Check if ffmpeg is available (don't download static binary - causes crashes on some systems)
    let ffmpeg_check = tokio::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .await;
    
    if ffmpeg_check.is_err() || !ffmpeg_check.unwrap().status.success() {
        eprintln!("\n⚠️  WARNING: ffmpeg not found!");
        eprintln!("ffmpeg is required for time range downloads and audio conversion.");
        eprintln!("\nPlease install ffmpeg for your system:");
        eprintln!("\n📦 Installation Instructions:");
        eprintln!("  • macOS (Homebrew): brew install ffmpeg");
        eprintln!("  • Debian/Ubuntu:    sudo apt install ffmpeg");
        eprintln!("  • Arch Linux:       sudo pacman -S ffmpeg");
        eprintln!("  • Fedora:           sudo dnf install ffmpeg");
        eprintln!("  • Windows:          Download from https://ffmpeg.org");
        eprintln!("\nThe app will still work for basic downloads using the browser's cookies if configured.\n");
    }

    // Download Rust PO Token provider for YouTube authentication
    // Dynamically select binary based on OS/Arch
    let pot_url = if cfg!(target_os = "macos") {
         if cfg!(target_arch = "aarch64") {
             "https://github.com/jim60105/bgutil-ytdlp-pot-provider-rs/releases/download/v0.6.4/bgutil-pot-macos-aarch64"
         } else {
             "https://github.com/jim60105/bgutil-ytdlp-pot-provider-rs/releases/download/v0.6.4/bgutil-pot-macos-x86_64"
         }
    } else if cfg!(target_os = "windows") {
         "https://github.com/jim60105/bgutil-ytdlp-pot-provider-rs/releases/download/v0.6.4/bgutil-pot-windows-x86_64.exe"
    } else {
         // Assume Linux
         if cfg!(target_arch = "aarch64") {
             "https://github.com/jim60105/bgutil-ytdlp-pot-provider-rs/releases/download/v0.6.4/bgutil-pot-linux-aarch64"
         } else {
             "https://github.com/jim60105/bgutil-ytdlp-pot-provider-rs/releases/download/v0.6.4/bgutil-pot-linux-x86_64"
         }
    };

    let pot_response = reqwest::get(pot_url).await;
    
    if let Ok(response) = pot_response {
        if let Ok(bytes) = response.bytes().await {
            let pot_path = bin_dir.join(if cfg!(target_os = "windows") { "bgutil-pot.exe" } else { "bgutil-pot" });
            if let Ok(mut file) = tokio::fs::File::create(&pot_path).await {
                let _ = file.write_all(&bytes).await;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = file.metadata().await {
                        let mut perms = metadata.permissions();
                        perms.set_mode(0o755);
                        let _ = file.set_permissions(perms).await;
                    }
                }
            }
        }
    }

    Ok(())
}

// Fetch thumbnail
async fn fetch_thumbnail(url: String) -> Result<image::Handle, String> {
    let bin_dir = get_bin_dir();
    let local_yt = bin_dir.join(if cfg!(target_os = "windows") { "yt-dlp.exe" } else { "yt-dlp" });
    let cmd_str = if local_yt.exists() {
        local_yt.to_string_lossy().to_string()
    } else {
        if cfg!(target_os = "windows") { "yt-dlp.exe".to_string() } else { "yt-dlp".to_string() }
    };
    
    let output = tokio::process::Command::new(cmd_str)
        .arg("--print")
        .arg("thumbnail")
        .arg("--flat-playlist")
        .arg("--skip-download")
        .arg(&url)
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let thumb_url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if thumb_url.is_empty() { return Err("No thumbnail found".to_string()); }

    let bytes = reqwest::get(&thumb_url).await.map_err(|e| e.to_string())?.bytes().await.map_err(|e| e.to_string())?;
    Ok(image::Handle::from_bytes(bytes.to_vec()))
}

// Fetch video info (title and duration)
async fn fetch_video_info(url: String) -> Result<(String, f32), String> {
    let bin_dir = get_bin_dir();
    let local_yt = bin_dir.join(if cfg!(target_os = "windows") { "yt-dlp.exe" } else { "yt-dlp" });
    let cmd_str = if local_yt.exists() {
        local_yt.to_string_lossy().to_string()
    } else {
        if cfg!(target_os = "windows") { "yt-dlp.exe".to_string() } else { "yt-dlp".to_string() }
    };
    
    let output = tokio::process::Command::new(cmd_str)
        .arg("--print")
        .arg("%(title)s|||%(duration)s")
        .arg("--flat-playlist")
        .arg("--skip-download")
        .arg(&url)
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let result = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parts: Vec<&str> = result.split("|||").collect();
    
    if parts.len() < 2 {
        return Err("Failed to fetch video info".to_string());
    }
    
    let title = parts[0].to_string();
    let duration: f32 = parts[1].parse().unwrap_or(0.0);
    
    Ok((title, duration))
}

// Download a single queue item
async fn download_queue_item(
    item: QueueItem,
    folder: PathBuf,
    settings: AdvancedSettings
) -> Result<(), String> {
    let bin_dir = get_bin_dir();
    let local_yt = bin_dir.join(if cfg!(target_os = "windows") { "yt-dlp.exe" } else { "yt-dlp" });
    let local_ffmpeg = bin_dir.join(if cfg!(target_os = "windows") { "ffmpeg.exe" } else { "ffmpeg" });
    
    let (cmd_str, use_local_ffmpeg) = if local_yt.exists() {
        (local_yt.to_string_lossy().to_string(), local_ffmpeg.exists())
    } else {
        (if cfg!(target_os = "windows") { "yt-dlp.exe".to_string() } else { "yt-dlp".to_string() }, false)
    };
    
    let mut cmd = tokio::process::Command::new(&cmd_str);
    
    
    // Always add bin/ to PATH so yt-dlp can find ffmpeg (needed for time range downloads)
    // Always add bin/ to PATH so yt-dlp can find ffmpeg (needed for time range downloads)
    let current_path = std::env::var("PATH").unwrap_or_default();
    let bin_path_env = get_bin_dir();
    let path_sep = if cfg!(target_os = "windows") { ";" } else { ":" };
    cmd.env("PATH", format!("{}{}{}", bin_path_env.display(), path_sep, current_path));


    cmd.arg(&item.url)
        .arg("-o")
        .arg("%(title)s.%(ext)s")
        .arg("--newline")
        .arg("--no-playlist")
        .current_dir(&folder)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Media type and format
    match item.media_type {
        MediaType::Video => {
            cmd.arg("-f").arg("bestvideo+bestaudio/best");
            let format_str = match item.output_format {
                OutputFormat::MP4 => "mp4",
                OutputFormat::MKV => "mkv",
                OutputFormat::WEBM => "webm",
                _ => "mp4",
            };
            cmd.arg("--merge-output-format").arg(format_str);
        }
        MediaType::Audio => {
            cmd.arg("-x");
            let format_str = match item.output_format {
                OutputFormat::MP3 => "mp3",
                OutputFormat::M4A => "m4a",
                OutputFormat::OPUS => "opus",
                OutputFormat::FLAC => "flac",
                _ => "mp3",
            };
            cmd.arg("--audio-format").arg(format_str);
        }
    }

    // Time range
    if let Some(range) = &item.time_range {
        let range_str = format_time_range_for_ytdlp(range);
        cmd.arg("--download-sections").arg(range_str);
    }
    
    let is_audio = item.media_type == MediaType::Audio;
    let should_embed_manual = is_audio && settings.embed_thumbnail;

    // Advanced settings
    if settings.embed_subs {
        cmd.arg("--write-subs").arg("--embed-subs");
    }
    
    if settings.embed_thumbnail {
        if should_embed_manual {
             cmd.arg("--write-thumbnail").arg("--convert-thumbnails").arg("jpg");
        } else {
             cmd.arg("--embed-thumbnail");
        }
    }
    
    if settings.restrict_filenames {
        cmd.arg("--restrict-filenames");
    }
    if !settings.proxy_url.is_empty() {
        cmd.arg("--proxy").arg(&settings.proxy_url);
    }
    if let Some(browser) = settings.cookies_browser {
        cmd.arg("--cookies-from-browser").arg(format!("{:?}", browser).to_lowercase());
    }

    if let Some(crop) = &item.crop_selection {
        let filter = format!("crop=iw*{:.4}:ih*{:.4}:iw*{:.4}:ih*{:.4}", 
                             crop.width, crop.height, crop.x, crop.y);

        let ffmpeg_bin = if use_local_ffmpeg {
             get_bin_dir().join(if cfg!(target_os = "windows") { "ffmpeg.exe" } else { "ffmpeg" }).to_string_lossy().to_string()
        } else {
            if cfg!(target_os = "windows") { "ffmpeg.exe".to_string() } else { "ffmpeg".to_string() }
        };
        
        let vcodec = match item.output_format {
            OutputFormat::WEBM => "libvpx-vp9",
            _ => "libx264",
        };
        
        // Use --exec to reliably crop after download/trim
        // Note: {{}} is escaped {} for Rust format string.
        let exec_cmd = if cfg!(target_os = "windows") {
             format!(
                "{} -y -i {{}} -vf \"{}\" -c:v {} -c:a copy {{}}.cropped.mp4 && move /y {{}}.cropped.mp4 {{}}",
                ffmpeg_bin, filter, vcodec
            )
        } else {
             format!(
                "{} -y -i {{}} -vf \"{}\" -c:v {} -c:a copy {{}}.cropped.mp4 && mv {{}}.cropped.mp4 {{}}",
                ffmpeg_bin, filter, vcodec
            )
        };
        
        cmd.arg("--exec").arg(exec_cmd);
    }

    // DEBUG: Print the command being executed
    eprintln!("=== YT-DLP COMMAND ===");
    eprintln!("Command: {:?}", cmd);
    eprintln!("=====================");

    let child = cmd.spawn().map_err(|e| e.to_string())?;
    
    let output = child.wait_with_output().await.map_err(|e| e.to_string())?;
    
    if output.status.success() {
        if should_embed_manual {
             // Find filename in stdout
             let stdout = String::from_utf8_lossy(&output.stdout);
             let mut filename = None;
             for line in stdout.lines() {
                 if let Some(idx) = line.find("Destination: ") {
                     filename = Some(line[idx + 13..].trim().to_string());
                 } else if let Some(idx) = line.find("Merging formats into \"") {
                      let end_idx = line.rfind("\"").unwrap_or(line.len());
                      if end_idx > idx + 22 {
                          filename = Some(line[idx + 22..end_idx].trim().to_string());
                      }
                 }
             }
             
             if let Some(fname) = filename {
                 let p = folder.join(fname);
                 embed_thumbnail_lofty(&p);
             }
        }
        Ok(())
    } else {
        Err("Download failed".to_string())
    }
}

// Quick download subscription
// Quick download subscription (wrapper)
fn download_subscription(
    url: String, 
    folder: PathBuf, 
    format: DownloadFormat, 
    settings: AdvancedSettings,
    time_range: Option<TimeRange>
) -> Subscription<Message> {
    let args = DownloadArgs {
        url,
        folder,
        format,
        settings,
        time_range,
    };
    
    iced::Subscription::run_with(args, create_download_stream)
}

#[derive(Debug, Clone)]
struct DownloadArgs {
    url: String,
    folder: PathBuf,
    format: DownloadFormat,
    settings: AdvancedSettings,
    time_range: Option<TimeRange>,
}

impl PartialEq for DownloadArgs {
    fn eq(&self, _other: &Self) -> bool {
        // Treat all instances as equal to maintain subscription identity
        // regardless of wrapper updates, mimicking static ID behavior
        true
    }
}

impl Eq for DownloadArgs {}

impl std::hash::Hash for DownloadArgs {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Use a constant hash to maintain stable identity
        "download_subscription".hash(state);
    }
}

fn create_download_stream(args: &DownloadArgs) -> impl iced::futures::Stream<Item = Message> {
    let url = args.url.clone();
    let folder = args.folder.clone();
    let format = args.format;
    let settings = args.settings.clone();
    let time_range = args.time_range.clone();

    iced::stream::channel(100, move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
        let mut state = DownloadState::Ready(url, folder, format, settings, time_range);
        loop {
            state = match state {
                DownloadState::Ready(url, folder, format, settings, time_range) => {
                    let bin_dir = get_bin_dir();
                    let local_yt = bin_dir.join(if cfg!(target_os = "windows") { "yt-dlp.exe" } else { "yt-dlp" });
                    let local_ffmpeg = bin_dir.join(if cfg!(target_os = "windows") { "ffmpeg.exe" } else { "ffmpeg" });
                    
                    let (cmd_str, _use_local_ffmpeg) = if local_yt.exists() {
                        (local_yt.to_string_lossy().to_string(), local_ffmpeg.exists())
                    } else {
                        (if cfg!(target_os = "windows") { "yt-dlp.exe".to_string() } else { "yt-dlp".to_string() }, false)
                    };
                    
                    let mut cmd = tokio::process::Command::new(&cmd_str);
                    
                    // Always add bin/ to PATH so yt-dlp can find ffmpeg (needed for time range downloads)
                    // Always add bin/ to PATH so yt-dlp can find ffmpeg (needed for time range downloads)
                    let current_path = std::env::var("PATH").unwrap_or_default();
                    let bin_path_env = get_bin_dir();
                    let path_sep = if cfg!(target_os = "windows") { ";" } else { ":" };
                    cmd.env("PATH", format!("{}{}{}", bin_path_env.display(), path_sep, current_path));


                    cmd.arg(&url).arg("-o").arg("%(title)s.%(ext)s").arg("--newline").arg("--no-playlist")
                       .current_dir(&folder).stdout(Stdio::piped()).stderr(Stdio::piped());

                    match format {
                        DownloadFormat::VideoBest => { cmd.arg("-f").arg("bestvideo+bestaudio/best"); },
                        DownloadFormat::Video1080p => { cmd.arg("-f").arg("bestvideo[height<=1080]+bestaudio/best[height<=1080]"); },
                        DownloadFormat::Video720p => { cmd.arg("-f").arg("bestvideo[height<=720]+bestaudio/best[height<=720]"); },
                        DownloadFormat::AudioBest => { cmd.arg("-x"); }, 
                        DownloadFormat::AudioMp3 => { cmd.arg("-x").arg("--audio-format").arg("mp3"); },
                    }
                    if !matches!(format, DownloadFormat::AudioMp3 | DownloadFormat::AudioBest) { cmd.arg("--merge-output-format").arg("mp4"); }

                    let is_audio = matches!(format, DownloadFormat::AudioMp3 | DownloadFormat::AudioBest);
                    let should_embed_manual = is_audio && settings.embed_thumbnail;

                    if settings.embed_subs { cmd.arg("--write-subs").arg("--embed-subs"); }
                    
                    if settings.embed_thumbnail {
                        if should_embed_manual {
                            cmd.arg("--write-thumbnail").arg("--convert-thumbnails").arg("jpg");
                        } else {
                            cmd.arg("--embed-thumbnail");
                        }
                    }
                    
                    if settings.restrict_filenames { cmd.arg("--restrict-filenames"); }
                    if !settings.proxy_url.is_empty() { cmd.arg("--proxy").arg(&settings.proxy_url); }
                    if let Some(browser) = settings.cookies_browser { cmd.arg("--cookies-from-browser").arg(format!("{:?}", browser).to_lowercase()); }
                    
                    // Apply time range if specified
                    if let Some(range) = time_range {
                        let range_str = format_time_range_for_ytdlp(&range);
                        cmd.arg("--download-sections").arg(range_str);
                    }
                    
                    // DEBUG: Print the command being executed
                    eprintln!("=== BATCH YT-DLP COMMAND ===");
                    eprintln!("Command: {:?}", cmd);
                    eprintln!("============================");
                    
                    match cmd.spawn() {
                        Ok(mut child) => {
                            let stdout = child.stdout.take().unwrap();
                            let reader = BufReader::new(stdout);
                            let _ = output.send(Message::DownloadProgress(DownloadEvent::Starting)).await;
                            DownloadState::Running(child, reader, None, should_embed_manual, folder.clone())
                        }
                        Err(e) => {
                            let _ = output.send(Message::DownloadProgress(DownloadEvent::Finished(Err(e.to_string())))).await;
                            DownloadState::Finished
                        }
                    }
                }
                DownloadState::Running(mut child, mut reader, mut filename, should_embed_manual, folder) => {
                    let mut line = String::new();
                    match reader.read_line(&mut line).await {
                        Ok(0) => {
                            let status = child.wait().await;
                             match status {
                                Ok(s) if s.success() => {
                                    if should_embed_manual {
                                        if let Some(fname) = filename {
                                            let p = folder.join(fname);
                                            embed_thumbnail_lofty(&p);
                                        }
                                    }
                                    
                                    let _ = output.send(Message::DownloadProgress(DownloadEvent::Finished(Ok(())))).await;
                                    DownloadState::Finished
                                },
                                Ok(s) => {
                                    let _ = output.send(Message::DownloadProgress(DownloadEvent::Finished(Err(format!("Process failed: {}", s))))).await;
                                    DownloadState::Finished
                                },
                                Err(e) => {
                                    let _ = output.send(Message::DownloadProgress(DownloadEvent::Finished(Err(e.to_string())))).await;
                                    DownloadState::Finished
                                },
                            }
                        }
                        Ok(_) => {
                            // Find filename
                            if let Some(idx) = line.find("Destination: ") {
                                let f = line[idx + 13..].trim().to_string();
                                filename = Some(f);
                            } else if let Some(idx) = line.find("Merging formats into \"") {
                                 let end_idx = line.rfind("\"").unwrap_or(line.len());
                                 if end_idx > idx + 22 {
                                     let f = line[idx + 22..end_idx].trim().to_string();
                                     filename = Some(f);
                                 }
                            }
                            
                            // Parse yt-dlp output: [download]  45.3% of 10.50MiB at 1.23MiB/s ETA 00:05
                            let re_percent = Regex::new(r"(\d+\.?\d*)%").unwrap();
                            let re_size = Regex::new(r"of\s+([\d.]+\s*[KMG]i?B)").unwrap();
                            let re_speed = Regex::new(r"at\s+([\d.]+\s*[KMG]i?B/s)").unwrap();
                            let re_eta = Regex::new(r"ETA\s+([\d:]+)").unwrap();
                            
                            let progress = if let Some(caps) = re_percent.captures(&line) {
                                caps.get(1).map_or(0.0, |m| m.as_str().parse().unwrap_or(0.0))
                            } else { -1.0 };
                            
                            // Build detailed status message
                            let mut status_parts = Vec::new();
                            if let Some(caps) = re_size.captures(&line) {
                                if let Some(size) = caps.get(1) {
                                    status_parts.push(format!("Size: {}", size.as_str()));
                                }
                            }
                            if let Some(caps) = re_speed.captures(&line) {
                                if let Some(speed) = caps.get(1) {
                                    status_parts.push(format!("Speed: {}", speed.as_str()));
                                }
                            }
                            if let Some(caps) = re_eta.captures(&line) {
                                if let Some(eta) = caps.get(1) {
                                    status_parts.push(format!("ETA: {}", eta.as_str()));
                                }
                            }
                            
                            let status_msg = if !status_parts.is_empty() {
                                status_parts.join(" • ")
                            } else {
                                line.trim().to_string()
                            };
                            
                            let _ = output.send(Message::DownloadProgress(DownloadEvent::Progress(if progress < 0.0 { 0.0 } else { progress }, status_msg))).await;
                            DownloadState::Running(child, reader, filename, should_embed_manual, folder)
                        }
                        Err(e) => {
                            let _ = output.send(Message::DownloadProgress(DownloadEvent::Finished(Err(e.to_string())))).await;
                            DownloadState::Finished
                        }
                    }
                }
                DownloadState::Finished => { 
                    iced::futures::future::pending().await 
                }
            };
        }
    })
}

enum DownloadState {
    Ready(String, PathBuf, DownloadFormat, AdvancedSettings, Option<TimeRange>),
    Running(tokio::process::Child, BufReader<tokio::process::ChildStdout>, Option<String>, bool, PathBuf),
    Finished,
}

fn embed_thumbnail_lofty(audio_path: &Path) {
    let stem = audio_path.file_stem().unwrap_or_default();
    let parent = audio_path.parent().unwrap_or(Path::new("."));
    
    let exts = ["jpg", "jpeg", "png", "webp"];
    let mut thumb_path = None;
    
    for ext in exts {
        let p = parent.join(format!("{}.{}", stem.to_string_lossy(), ext));
        if p.exists() {
            thumb_path = Some(p);
            break;
        }
    }
    
    if let Some(tp) = thumb_path {
        if let Ok(mut tagged_file) = lofty::read_from_path(audio_path) {
            if let Ok(image_data) = std::fs::read(&tp) {
                 let mime = match tp.extension().and_then(|e| e.to_str()) {
                     Some("png") => MimeType::Png,
                     Some("jpg") | Some("jpeg") => MimeType::Jpeg,
                     _ => MimeType::Jpeg 
                 };
                 
                 let picture = Picture::new_unchecked(
                     PictureType::CoverFront,
                     Some(mime),
                     Some("Thumbnail".to_string()),
                     image_data
                 );
                 
                 if let Some(tag) = tagged_file.primary_tag_mut() {
                     tag.push_picture(picture);
                     let _ = tag.save_to_path(audio_path, WriteOptions::default());
                 } else {
                     let primary_type = tagged_file.file_type().primary_tag_type();
                     let mut tag = Tag::new(primary_type);
                     tag.push_picture(picture);
                     let _ = tag.save_to_path(audio_path, WriteOptions::default());
                 }
                 
                 let _ = std::fs::remove_file(tp);
            }
        }
    }
}

// ============================================================================
// CANVAS WIDGETS
// ============================================================================

#[derive(Debug, Default)]
struct AnimatedLogo {
    tick: f32,
}

impl canvas::Program<Message> for AnimatedLogo {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = frame.center();
        let radius = bounds.width.min(bounds.height) / 2.0;

        // Hexagon Path
        let hexagon = canvas::Path::new(|p| {
            for i in 0..6 {
                let angle = (60.0 * i as f32 + 30.0).to_radians();
                let x = center.x + radius * angle.cos();
                let y = center.y + radius * angle.sin();
                if i == 0 { p.move_to(Point::new(x, y)); } else { p.line_to(Point::new(x, y)); }
            }
            p.close();
        });

        // 1. Base Gradient
        let base_stops = [
            Some(iced::gradient::ColorStop { offset: 0.0, color: Color::from_rgb(0.2, 0.1, 0.4) }),
            Some(iced::gradient::ColorStop { offset: 1.0, color: Color::from_rgb(0.5, 0.2, 0.8) }),
            None, None, None, None, None, None
        ];
        let base_gradient = canvas::Gradient::Linear(canvas::gradient::Linear {
            start: Point::new(bounds.x, bounds.y),
            end: Point::new(bounds.x + bounds.width, bounds.y + bounds.height),
            stops: base_stops
        });
        frame.fill(&hexagon, base_gradient);

        // 2. Light Panning Animation
        let loop_time = 3.0;
        let t = (self.tick % loop_time) / loop_time;
        // Panning from left (-width) to right (+2*width) to ensure full sweep
        let pan_x = (t * 3.0 - 1.0) * bounds.width;
        
        let shine_stops = [
            Some(iced::gradient::ColorStop { offset: 0.0, color: Color::from_rgba(1.0, 1.0, 1.0, 0.0) }),
            Some(iced::gradient::ColorStop { offset: 0.5, color: Color::from_rgba(1.0, 1.0, 1.0, 0.3) }),
            Some(iced::gradient::ColorStop { offset: 1.0, color: Color::from_rgba(1.0, 1.0, 1.0, 0.0) }),
            None, None, None, None, None
        ];
        let shine_gradient = canvas::Gradient::Linear(canvas::gradient::Linear {
            start: Point::new(bounds.x + pan_x - bounds.width * 0.5, bounds.y),
            end: Point::new(bounds.x + pan_x + bounds.width * 0.5, bounds.y),
            stops: shine_stops
        });
        // Overlay shine on the hexagon shape
        frame.fill(&hexagon, shine_gradient);

        // 3. Play Triangle
        let triangle = canvas::Path::new(|p| {
            let offset_x = radius * 0.08;
            p.move_to(Point::new(center.x - radius * 0.25 + offset_x, center.y - radius * 0.35));
            p.line_to(Point::new(center.x - radius * 0.25 + offset_x, center.y + radius * 0.35));
            p.line_to(Point::new(center.x + radius * 0.45 + offset_x, center.y));
            p.close();
        });
        frame.fill(&triangle, Color::WHITE);
        
        // 4. Stroke
        frame.stroke(&hexagon, canvas::Stroke {
            style: canvas::Style::Solid(Color::from_rgb(0.7, 0.4, 1.0)),
            width: 2.0,
            ..Default::default()
        });

        vec![frame.into_geometry()]
    }
}

#[derive(Debug)]
struct ProgressBar { progress: f32, tick: f32 }

impl canvas::Program<Message> for ProgressBar {
    type State = ();
    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let background = canvas::Path::rectangle(Point::ORIGIN, bounds.size());
        frame.fill(&background, Color::from_rgb(0.15, 0.15, 0.2));
        if self.progress > 0.0 {
            let width = bounds.width * (self.progress / 100.0);
            let bar = canvas::Path::rectangle(Point::ORIGIN, Size::new(width, bounds.height));
            let shift = (self.tick * 2.0).sin() * 0.2;
            let stops = [ Some(iced::gradient::ColorStop { offset: 0.0, color: Color::from_rgb(0.4 + shift, 0.1, 0.8).into() }), Some(iced::gradient::ColorStop { offset: 0.5, color: Color::from_rgb(0.2, 0.6, 1.0).into() }), Some(iced::gradient::ColorStop { offset: 1.0, color: Color::from_rgb(0.8, 0.2, 0.9).into() }), None, None, None, None, None ];
            let gradient = canvas::Gradient::Linear(canvas::gradient::Linear { start: Point::new(0.0, 0.0), end: Point::new(bounds.width, 0.0), stops });
            frame.fill(&bar, gradient);
        }
        vec![frame.into_geometry()]
    }
}

#[derive(Debug, Default)]
struct ErrorIcon;

impl canvas::Program<Message> for ErrorIcon {
    type State = ();
    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = frame.center();
        let radius = bounds.width.min(bounds.height) / 2.0 * 0.8;

        // Outer circle
        let circle = canvas::Path::circle(center, radius);
        frame.stroke(&circle, canvas::Stroke { 
            style: canvas::Style::Solid(Color::from_rgb(1.0, 0.4, 0.4)), 
            width: 4.0, 
            ..Default::default() 
        });
        
        // Exclamation mark
        let mark_top = canvas::Path::rectangle(
            Point::new(center.x - 3.0, center.y - radius * 0.5),
            Size::new(6.0, radius * 0.6)
        );
        frame.fill(&mark_top, Color::from_rgb(1.0, 0.4, 0.4));
        
        let mark_dot = canvas::Path::circle(
            Point::new(center.x, center.y + radius * 0.4),
            4.0
        );
        frame.fill(&mark_dot, Color::from_rgb(1.0, 0.4, 0.4));
        
        vec![frame.into_geometry()]
    }
}

#[derive(Debug)]
struct RunningCharacter {
    progress: f32,  // 0.0 to 1.0
    tick: f32,
}

impl canvas::Program<Message> for RunningCharacter {
    type State = ();
    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        
        // Character position (moves from left to right)
        // Add margin so character doesn't go off screen
        let margin = 30.0;
        let x_pos = margin + (bounds.width - 2.0 * margin) * self.progress;
        
        // Smoother bouncing effect
        let bounce = (self.tick * 5.0).sin().abs() * 8.0;
        let y_pos = bounds.height / 2.0 + 10.0; // Center vertically with offset
        
        // Stick figure - larger and more visible
        let head_radius = 10.0;
        let head = canvas::Path::circle(Point::new(x_pos, y_pos - 25.0 - bounce), head_radius);
        frame.fill(&head, Color::from_rgb(0.6, 0.3, 0.9));
        
        // Body
        let body = canvas::Path::line(
            Point::new(x_pos, y_pos - 15.0 - bounce),
            Point::new(x_pos, y_pos + 10.0 - bounce)
        );
        frame.stroke(&body, canvas::Stroke {
            style: canvas::Style::Solid(Color::from_rgb(0.6, 0.3, 0.9)),
            width: 3.5,
            ..Default::default()
        });
        
        // Arms (animated) - smoother movement
        let arm_angle = (self.tick * 4.0).sin() * 0.6;
        let left_arm_end = Point::new(
            x_pos - 14.0 * arm_angle.cos(),
            y_pos - 8.0 - bounce + 14.0 * arm_angle.sin()
        );
        let right_arm_end = Point::new(
            x_pos + 14.0 * arm_angle.cos(),
            y_pos - 8.0 - bounce - 14.0 * arm_angle.sin()
        );
        let left_arm = canvas::Path::line(Point::new(x_pos, y_pos - 8.0 - bounce), left_arm_end);
        let right_arm = canvas::Path::line(Point::new(x_pos, y_pos - 8.0 - bounce), right_arm_end);
        
        frame.stroke(&left_arm, canvas::Stroke {
            style: canvas::Style::Solid(Color::from_rgb(0.6, 0.3, 0.9)),
            width: 3.0,
            ..Default::default()
        });
        frame.stroke(&right_arm, canvas::Stroke {
            style: canvas::Style::Solid(Color::from_rgb(0.6, 0.3, 0.9)),
            width: 3.0,
            ..Default::default()
        });
        
        // Legs (animated) - smoother movement
        let leg_angle = (self.tick * 4.0).sin() * 0.8;
        let left_leg_end = Point::new(
            x_pos - 12.0 * leg_angle.cos(),
            y_pos + 25.0 - bounce + 8.0 * leg_angle.sin().abs()
        );
        let right_leg_end = Point::new(
            x_pos + 12.0 * leg_angle.cos(),
            y_pos + 25.0 - bounce + 8.0 * (-leg_angle).sin().abs()
        );
        let left_leg = canvas::Path::line(Point::new(x_pos, y_pos + 10.0 - bounce), left_leg_end);
        let right_leg = canvas::Path::line(Point::new(x_pos, y_pos + 10.0 - bounce), right_leg_end);
        
        frame.stroke(&left_leg, canvas::Stroke {
            style: canvas::Style::Solid(Color::from_rgb(0.6, 0.3, 0.9)),
            width: 3.0,
            ..Default::default()
        });
        frame.stroke(&right_leg, canvas::Stroke {
            style: canvas::Style::Solid(Color::from_rgb(0.6, 0.3, 0.9)),
            width: 3.0,
            ..Default::default()
        });
        
        vec![frame.into_geometry()]
    }
}

#[derive(Debug)]
struct Stopwatch {
    elapsed: f32,  // Seconds elapsed
}

impl canvas::Program<Message> for Stopwatch {
    type State = ();
    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let center = frame.center();
        let radius = bounds.width.min(bounds.height) / 2.0 * 0.85;
        
        // Outer circle (clock face)
        let circle = canvas::Path::circle(center, radius);
        frame.stroke(&circle, canvas::Stroke {
            style: canvas::Style::Solid(Color::from_rgb(0.6, 0.3, 0.9)),
            width: 3.0,
            ..Default::default()
        });
        
        // Inner circle (smaller)
        let inner_circle = canvas::Path::circle(center, radius * 0.9);
        frame.stroke(&inner_circle, canvas::Stroke {
            style: canvas::Style::Solid(Color::from_rgb(0.5, 0.25, 0.8)),
            width: 1.5,
            ..Default::default()
        });
        
        // Hour markers (12, 3, 6, 9)
        for i in 0..4 {
            let angle = (i as f32) * std::f32::consts::PI / 2.0 - std::f32::consts::PI / 2.0;
            let start_radius = radius * 0.85;
            let end_radius = radius * 0.75;
            let start = Point::new(
                center.x + start_radius * angle.cos(),
                center.y + start_radius * angle.sin()
            );
            let end = Point::new(
                center.x + end_radius * angle.cos(),
                center.y + end_radius * angle.sin()
            );
            let marker = canvas::Path::line(start, end);
            frame.stroke(&marker, canvas::Stroke {
                style: canvas::Style::Solid(Color::from_rgb(0.6, 0.3, 0.9)),
                width: 2.5,
                ..Default::default()
            });
        }
        
        // Second hand (rotating)
        let seconds = self.elapsed % 60.0;
        let second_angle = (seconds / 60.0) * 2.0 * std::f32::consts::PI - std::f32::consts::PI / 2.0;
        let second_hand_end = Point::new(
            center.x + radius * 0.7 * second_angle.cos(),
            center.y + radius * 0.7 * second_angle.sin()
        );
        let second_hand = canvas::Path::line(center, second_hand_end);
        frame.stroke(&second_hand, canvas::Stroke {
            style: canvas::Style::Solid(Color::from_rgb(0.9, 0.4, 1.0)),
            width: 2.0,
            ..Default::default()
        });
        
        // Minute hand (slower rotation)
        let minutes = (self.elapsed / 60.0) % 60.0;
        let minute_angle = (minutes / 60.0) * 2.0 * std::f32::consts::PI - std::f32::consts::PI / 2.0;
        let minute_hand_end = Point::new(
            center.x + radius * 0.5 * minute_angle.cos(),
            center.y + radius * 0.5 * minute_angle.sin()
        );
        let minute_hand = canvas::Path::line(center, minute_hand_end);
        frame.stroke(&minute_hand, canvas::Stroke {
            style: canvas::Style::Solid(Color::from_rgb(0.7, 0.35, 0.95)),
            width: 3.0,
            ..Default::default()
        });
        
        // Center dot
        let center_dot = canvas::Path::circle(center, 4.0);
        frame.fill(&center_dot, Color::from_rgb(0.8, 0.4, 1.0));
        
        vec![frame.into_geometry()]
    }
}

#[derive(Debug)]
struct EnhancedETA {
    eta_seconds: u32,
    progress: f32,  // 0.0 to 100.0
    tick: f32,
}

impl canvas::Program<Message> for EnhancedETA {
    type State = ();
    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        
        // Determine color based on time remaining
        let (bg_color, text_color) = if self.eta_seconds > 60 {
            // Green zone (> 1 minute)
            (Color::from_rgb(0.1, 0.4, 0.2), Color::from_rgb(0.3, 0.9, 0.5))
        } else if self.eta_seconds > 30 {
            // Yellow zone (30-60 seconds)
            (Color::from_rgb(0.4, 0.4, 0.1), Color::from_rgb(0.9, 0.9, 0.3))
        } else if self.eta_seconds > 10 {
            // Orange zone (10-30 seconds)
            (Color::from_rgb(0.5, 0.3, 0.1), Color::from_rgb(1.0, 0.6, 0.2))
        } else {
            // Red zone (< 10 seconds)
            (Color::from_rgb(0.5, 0.1, 0.1), Color::from_rgb(1.0, 0.3, 0.3))
        };
        
        // Pulsing effect
        let pulse = (self.tick * 2.0).sin() * 0.1 + 0.9;
        
        // Background with gradient
        let background = canvas::Path::rectangle(Point::ORIGIN, bounds.size());
        frame.fill(&background, bg_color);
        
        // Pulsing border
        let border = canvas::Path::rectangle(Point::ORIGIN, bounds.size());
        frame.stroke(&border, canvas::Stroke {
            style: canvas::Style::Solid(Color {
                r: text_color.r * pulse,
                g: text_color.g * pulse,
                b: text_color.b * pulse,
                a: 1.0,
            }),
            width: 4.0,
            ..Default::default()
        });
        
        vec![frame.into_geometry()]
    }
}


// ============================================================================
// STYLES
// ============================================================================

struct MainBackground;
// MainBackground style - use inline styling in iced 0.14
// MainBackground style - use inline styling in iced 0.14
fn main_background_style(app_theme: AppTheme) -> container::Style {
    let bg = match app_theme {
        AppTheme::Default => Color::from_rgb(0.05, 0.05, 0.15),
        AppTheme::Vibrant => Color::from_rgb(0.05, 0.02, 0.05), // Deep Warm Black
    };
    container::Style {
        background: Some(iced::Background::Color(bg)),
        ..Default::default()
    }
}

struct CardStyle;
// CardStyle - use inline styling in iced 0.14
fn card_style(app_theme: AppTheme) -> container::Style {
    let (bg, border) = match app_theme {
        AppTheme::Default => (
            Color::from_rgba(0.08, 0.08, 0.12, 0.85),
            Color::from_rgba(0.3, 0.3, 0.4, 0.3)
        ),
        AppTheme::Vibrant => (
            Color::from_rgba(0.1, 0.05, 0.05, 0.85),
            Color::from_rgba(0.5, 0.3, 0.1, 0.3) // Gold/Orange tint
        ),
    };

    container::Style {
        background: Some(iced::Background::Color(bg)),
        border: iced::border::Border {
            color: border,
            width: 1.0,
            radius: 12.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.5),
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 16.0,
        },
        ..Default::default()
    }
}

struct QueueItemStyle;
// QueueItemStyle - use inline styling in iced 0.14
fn queue_item_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(Color::from_rgb(0.09, 0.09, 0.13))),
        border: iced::Border {
            color: Color::from_rgb(0.15, 0.15, 0.2),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

struct OnyxInput;

struct OnyxPrimaryButton { active: bool }

struct OnyxSecondaryButton;

struct OnyxPickList;

struct OnyxOverlay;

// Start PO Token server for YouTube authentication
async fn start_po_token_server() {
    use std::path::PathBuf;
    
    // Find binary using get_bin_dir helper or fallback
    let bin_path = get_bin_dir().join(if cfg!(target_os = "windows") { "bgutil-pot.exe" } else { "bgutil-pot" });

    if !bin_path.exists() {
        return; // Not downloaded yet, skip
    }
    
    // Check if server is already running by attempting to connect to the port
    if tokio::net::TcpStream::connect("127.0.0.1:8190").await.is_ok() {
        return; // Server already running
    }
    
    // Start server in background
    let _ = tokio::process::Command::new(&bin_path)
        .arg("server")
        .arg("--port")
        .arg("8190")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

// ============================================================================
// VIDEO PLAYER STYLES
// ============================================================================

struct DarkOverlay;

struct PlayerModalStyle;

struct VideoAreaStyle;

struct TimelineBackgroundStyle;

struct SelectionOverlayStyle;

struct TrimHandleStyle;

struct TrimSliderStyle;


// ============================================================================
// TIMELINE TRIMMER CANVAS PROGRAM
// ============================================================================

struct TimelineTrimmer {
    duration: f32,
    start: f32,
    end: f32,
    dragging: Option<TrimHandle>,
    hover: Option<TrimHandle>,
}

impl canvas::Program<Message> for TimelineTrimmer {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        
        let width = bounds.width;
        let height = bounds.height;
        let p_height = height; // Full height for handles
        
        // 1. Background Track (Gray)
        let track_height = 40.0;
        let track_y = (height - track_height) / 2.0;
        // Don't draw full track, draw unused parts as dimmed later compared to selection
        let track = canvas::Path::rectangle(Point::new(0.0, track_y), Size::new(width, track_height));
        frame.fill(&track, Color::from_rgb(0.15, 0.15, 0.15)); // Darker background
        
        if self.duration <= 0.0 {
             return vec![frame.into_geometry()];
        }
        
        // Calculate positions
        let start_x = (self.start / self.duration) * width;
        let end_x = (self.end / self.duration) * width;
        let handle_width = 20.0;
        
        // 2. Selection Region (Semitransparent Yellow) + Dimmed Outer
        let selection_width = (end_x - start_x).max(0.0);
        
        // Dimmed Left
        let dim_left = canvas::Path::rectangle(Point::new(0.0, track_y), Size::new(start_x, track_height));
        frame.fill(&dim_left, Color::from_rgba(0.0, 0.0, 0.0, 0.6));
        
        // Dimmed Right
        let dim_right = canvas::Path::rectangle(Point::new(end_x, track_y), Size::new(width - end_x, track_height));
        frame.fill(&dim_right, Color::from_rgba(0.0, 0.0, 0.0, 0.6));
        
        // Selection
        let selection = canvas::Path::rectangle(Point::new(start_x, track_y), Size::new(selection_width, track_height));
        frame.fill(&selection, Color::from_rgba(1.0, 0.84, 0.0, 0.1)); // Very Light Gold
        // Top and Bottom thick borders
        let border_top = canvas::Path::line(Point::new(start_x, track_y), Point::new(end_x, track_y));
        let border_bottom = canvas::Path::line(Point::new(start_x, track_y + track_height), Point::new(end_x, track_y + track_height));
        let border_stroke = canvas::Stroke::default().with_color(Color::from_rgb(1.0, 0.84, 0.0)).with_width(4.0);
        frame.stroke(&border_top, border_stroke.clone());
        frame.stroke(&border_bottom, border_stroke);

        // 3. Handles (Start and End) - Chevron Style
        let handle_color = Color::from_rgb(1.0, 0.84, 0.0); // Gold
        let hover_color = Color::from_rgb(1.0, 0.9, 0.4); // Light Gold
        
        let start_color = if self.dragging == Some(TrimHandle::Start) || self.hover == Some(TrimHandle::Start) { hover_color } else { handle_color };
        let end_color = if self.dragging == Some(TrimHandle::End) || self.hover == Some(TrimHandle::End) { hover_color } else { handle_color };
        
        // Start Handle (Rounded Left, Flat Right)
        let start_handle_rect = canvas::Path::rounded_rectangle(
            Point::new(start_x - handle_width, 0.0), // Sits outside selection to left
            Size::new(handle_width, p_height),
            6.0.into()
        );
        frame.fill(&start_handle_rect, start_color);
        
        // Grip lines for Start
        let grip_center_x = start_x - handle_width / 2.0;    
        frame.stroke(&canvas::Path::line(Point::new(grip_center_x - 3.0, 12.0), Point::new(grip_center_x - 3.0, height - 12.0)), canvas::Stroke::default().with_color(Color::from_rgb(0.4, 0.3, 0.0)).with_width(2.0));
        frame.stroke(&canvas::Path::line(Point::new(grip_center_x + 3.0, 12.0), Point::new(grip_center_x + 3.0, height - 12.0)), canvas::Stroke::default().with_color(Color::from_rgb(0.4, 0.3, 0.0)).with_width(2.0));


        // End Handle (Flat Left, Rounded Right)
        let end_handle_rect = canvas::Path::rounded_rectangle(
             Point::new(end_x, 0.0), // Sits outside selection to right
             Size::new(handle_width, p_height),
             6.0.into()
        );
        frame.fill(&end_handle_rect, end_color);
        
        // Grip lines for End
        let grip_end_x = end_x + handle_width / 2.0;
        frame.stroke(&canvas::Path::line(Point::new(grip_end_x - 3.0, 12.0), Point::new(grip_end_x - 3.0, height - 12.0)), canvas::Stroke::default().with_color(Color::from_rgb(0.4, 0.3, 0.0)).with_width(2.0));
        frame.stroke(&canvas::Path::line(Point::new(grip_end_x + 3.0, 12.0), Point::new(grip_end_x + 3.0, height - 12.0)), canvas::Stroke::default().with_color(Color::from_rgb(0.4, 0.3, 0.0)).with_width(2.0));
        
        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let cursor_position = cursor.position_in(bounds)?;
        
        let width = bounds.width;
        let height = bounds.height;
        let handle_width = 20.0; // Matched to draw
        
        let time_to_x = |t: f32| -> f32 {
             if self.duration > 0.0 { (t / self.duration) * width } else { 0.0 }
        };
        
        let start_x = time_to_x(self.start);
        let end_x = time_to_x(self.end);
        
        // Define hit regions - Updated to match new Draw style
        // Start Handle: Left of start_x
        let start_handle_rect = Rectangle::new(
             Point::new(start_x - handle_width, 0.0),
             Size::new(handle_width, height)
        );
        // End Handle: Right of end_x
         let end_handle_rect = Rectangle::new(
             Point::new(end_x, 0.0),
             Size::new(handle_width, height)
        );
        
        match event {
            iced::Event::Mouse(mouse_event) => match mouse_event {
                 mouse::Event::ButtonPressed(mouse::Button::Left) => {
                      if start_handle_rect.contains(cursor_position) {
                           return Some(canvas::Action::publish(Message::TrimHandlePressed(TrimHandle::Start, cursor_position.x)));
                      } else if end_handle_rect.contains(cursor_position) {
                           return Some(canvas::Action::publish(Message::TrimHandlePressed(TrimHandle::End, cursor_position.x)));
                      } else if cursor_position.x > start_x && cursor_position.x < end_x {
                           return Some(canvas::Action::publish(Message::TrimHandlePressed(TrimHandle::Selection, cursor_position.x)));
                      }
                 }
                 mouse::Event::ButtonReleased(mouse::Button::Left) => {
                      if self.dragging.is_some() {
                           return Some(canvas::Action::publish(Message::TrimHandleReleased));
                      }
                 }
                 mouse::Event::CursorMoved { .. } => {
                      if self.dragging.is_some() {
                           return Some(canvas::Action::publish(Message::TrimHandleDragged(cursor_position.x)));
                      } else {
                           // Hover detection
                           if start_handle_rect.contains(cursor_position) {
                                return Some(canvas::Action::publish(Message::TrimHandleHover(Some(TrimHandle::Start))));
                           } else if end_handle_rect.contains(cursor_position) {
                                return Some(canvas::Action::publish(Message::TrimHandleHover(Some(TrimHandle::End))));
                           } else if cursor_position.x > start_x && cursor_position.x < end_x {
                                return Some(canvas::Action::publish(Message::TrimHandleHover(Some(TrimHandle::Selection))));
                           } else {
                                return Some(canvas::Action::publish(Message::TrimHandleHover(None)));
                           }
                      }
                 }
                 _ => {}
            }
            _ => {}
        }
        None
    }
}

// ============================================================================
// ANIMATED TAB BAR
// ============================================================================

struct AnimatedTabBar {
    active_tab: Tab,
    anim_pos: f32,
    queue_count: usize,
}

impl canvas::Program<Message> for AnimatedTabBar {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        
        let width = bounds.width;
        let height = bounds.height;
        let tab_width = width / 3.0; // 3 tabs
        
        // 1. Background Pill (Darker container)
        // Ensure consistent background for the whole bar
        let bg = canvas::Path::rounded_rectangle(Point::new(0.0, 0.0), Size::new(width, height), 12.0.into());
        frame.fill(&bg, Color::from_rgb(0.12, 0.12, 0.16));
        
        // 2. Sliding Active Indicator (Vibrant Purple/Blue Gradient look)
        // anim_pos ranges from 0.0 to 2.0
        let indicator_x = (self.anim_pos * tab_width).max(0.0).min(width - tab_width);
        
        let indicator_rect = canvas::Path::rounded_rectangle(
            Point::new(indicator_x + 4.0, 4.0), 
            Size::new(tab_width - 8.0, height - 8.0), 
            10.0.into()
        );
        frame.fill(&indicator_rect, Color::from_rgb(0.6, 0.3, 0.9)); // Purple
        
        // 3. Text Labels
        // "Quick Download"
        let quick_color = if self.anim_pos < 0.5 { Color::WHITE } else { Color::from_rgb(0.7, 0.7, 0.7) };
        frame.fill_text(canvas::Text {
            content: "⬇️ Download".to_string(),
            position: Point::new(tab_width / 2.0, height / 2.0),
            color: quick_color,
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            size: iced::Pixels(16.0),
            ..Default::default()
        });
        
        // "Batch Queue"
        let batch_color = if self.anim_pos >= 0.5 && self.anim_pos < 1.5 { Color::WHITE } else { Color::from_rgb(0.7, 0.7, 0.7) };
        frame.fill_text(canvas::Text {
            content: format!("📚 Queue ({})", self.queue_count),
            position: Point::new(tab_width * 1.5, height / 2.0),
            color: batch_color,
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            size: iced::Pixels(16.0),
            ..Default::default()
        });
        
        // "Settings"
        let settings_color = if self.anim_pos >= 1.5 { Color::WHITE } else { Color::from_rgb(0.7, 0.7, 0.7) };
        frame.fill_text(canvas::Text {
            content: "⚙️ Settings".to_string(),
            position: Point::new(tab_width * 2.5, height / 2.0),
            color: settings_color,
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: iced::alignment::Vertical::Center,
            size: iced::Pixels(16.0),
            ..Default::default()
        });
        
        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        use iced::mouse;
        
        if let iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(position) = cursor.position_in(bounds) {
                // Determine which tab was clicked
                let tab_width = bounds.width / 3.0; // 3 tabs
                if position.x < tab_width {
                    return Some(canvas::Action::publish(Message::SwitchTab(Tab::QuickDownload)));
                } else if position.x < tab_width * 2.0 {
                    return Some(canvas::Action::publish(Message::SwitchTab(Tab::BatchQueue)));
                } else {
                    return Some(canvas::Action::publish(Message::SwitchTab(Tab::Settings)));
                }
            }
        }
        None
    }
}

// ============================================================================
// GLASSMORPHISM STYLES
// ============================================================================

// ============================================================================
// GLASSMORPHISM STYLES
// ============================================================================

fn glass_primary_style(status: iced::widget::button::Status, app_theme: AppTheme) -> iced::widget::button::Style {
    let (active, hover, pressed) = match app_theme {
        AppTheme::Default => (
            Color::from_rgba(0.6, 0.3, 0.9, 0.7), // Purple glass
            Color::from_rgba(0.7, 0.4, 1.0, 0.8),
            Color::from_rgba(0.5, 0.2, 0.8, 0.9)
        ),
        AppTheme::Vibrant => (
            Color::from_rgba(0.0, 0.7, 0.7, 0.6), // Cyan glass
            Color::from_rgba(0.0, 0.8, 0.8, 0.7),
            Color::from_rgba(0.0, 0.6, 0.6, 0.8)
        ),
    };

    let bg_color = match status {
        iced::widget::button::Status::Active => active,
        iced::widget::button::Status::Hovered => hover,
        iced::widget::button::Status::Pressed => pressed,
        iced::widget::button::Status::Disabled => Color::from_rgba(0.3, 0.3, 0.3, 0.3),
    };

    iced::widget::button::Style {
        background: Some(iced::Background::Color(bg_color)),
        text_color: Color::WHITE,
        border: iced::border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.2),
            width: 1.0,
            radius: 20.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 10.0,
        },
        snap: false,
    }
}

fn glass_secondary_style(status: iced::widget::button::Status, app_theme: AppTheme) -> iced::widget::button::Style {
    let (active, hover) = match app_theme {
        AppTheme::Default => (
            Color::from_rgba(1.0, 1.0, 1.0, 0.1),
            Color::from_rgba(1.0, 1.0, 1.0, 0.15)
        ),
        AppTheme::Vibrant => (
            Color::from_rgba(1.0, 0.8, 0.2, 0.15), // Gold tint
            Color::from_rgba(1.0, 0.8, 0.2, 0.25)
        ),
    };

    let bg_color = match status {
        iced::widget::button::Status::Active => active,
        iced::widget::button::Status::Hovered => hover,
        iced::widget::button::Status::Pressed => Color::from_rgba(1.0, 1.0, 1.0, 0.05),
        iced::widget::button::Status::Disabled => Color::from_rgba(0.1, 0.1, 0.1, 0.1),
    };

    iced::widget::button::Style {
        background: Some(iced::Background::Color(bg_color)),
        text_color: Color::WHITE,
        border: iced::border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.1),
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

fn glass_danger_style(_theme: &iced::Theme, status: iced::widget::button::Status) -> iced::widget::button::Style {
    let bg_color = match status {
        iced::widget::button::Status::Active => Color::from_rgba(0.9, 0.3, 0.3, 0.7), // Red glass
        iced::widget::button::Status::Hovered => Color::from_rgba(1.0, 0.4, 0.4, 0.8),
        iced::widget::button::Status::Pressed => Color::from_rgba(0.8, 0.2, 0.2, 0.9),
        iced::widget::button::Status::Disabled => Color::from_rgba(0.3, 0.3, 0.3, 0.3),
    };

    iced::widget::button::Style {
        background: Some(iced::Background::Color(bg_color)),
        text_color: Color::WHITE,
        border: iced::border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.2),
            width: 1.0,
            radius: 20.0.into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 10.0,
        },
        snap: false,
    }
}

// ============================================================================
// CROP OVERLAY
// ============================================================================

struct CropOverlay {
    selection: Option<CropRect>,
    drag_start: Option<Point>,
}

impl canvas::Program<Message> for CropOverlay {
    type State = ();

    fn draw(&self, _state: &Self::State, renderer: &iced::Renderer, _theme: &Theme, bounds: Rectangle, _cursor: mouse::Cursor) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        
        let mut current_rect = None;
        if let Some(rect) = &self.selection {
             current_rect = Some(Rectangle {
                 x: rect.x * bounds.width,
                 y: rect.y * bounds.height,
                 width: rect.width * bounds.width,
                 height: rect.height * bounds.height,
             });
        }
        
        if let Some(rect) = current_rect {
             // Draw 4 distinct rectangles to create "hole" effect
             let top = Rectangle { x: 0.0, y: 0.0, width: bounds.width, height: rect.y };
             let bottom = Rectangle { x: 0.0, y: rect.y + rect.height, width: bounds.width, height: bounds.height - (rect.y + rect.height) };
             let left = Rectangle { x: 0.0, y: rect.y, width: rect.x, height: rect.height };
             let right = Rectangle { x: rect.x + rect.width, y: rect.y, width: bounds.width - (rect.x + rect.width), height: rect.height };
             
             for r in [top, bottom, left, right] {
                 if r.width > 0.0 && r.height > 0.0 {
                     let p = canvas::Path::rectangle(Point::new(r.x, r.y), Size::new(r.width, r.height));
                     frame.fill(&p, Color::from_rgba(0.0, 0.0, 0.0, 0.7));
                 }
             }
             
             // Draw Selection Border
             let border = canvas::Path::rectangle(Point::new(rect.x, rect.y), Size::new(rect.width, rect.height));
             frame.stroke(&border, canvas::Stroke {
                 style: canvas::Style::Solid(Color::WHITE),
                 width: 2.0,
                 ..Default::default()
             });
             
             // Grid lines (Rule of Thirds)
             let stroke = canvas::Stroke {
                 style: canvas::Style::Solid(Color::from_rgba(1.0, 1.0, 1.0, 0.3)),
                 width: 1.0,
                 ..Default::default()
             };
             
             // Vertical
             let v1 = canvas::Path::line(Point::new(rect.x + rect.width / 3.0, rect.y), Point::new(rect.x + rect.width / 3.0, rect.y + rect.height));
             let v2 = canvas::Path::line(Point::new(rect.x + 2.0 * rect.width / 3.0, rect.y), Point::new(rect.x + 2.0 * rect.width / 3.0, rect.y + rect.height));
             frame.stroke(&v1, stroke.clone());
             frame.stroke(&v2, stroke.clone());
             
             // Horizontal
             let h1 = canvas::Path::line(Point::new(rect.x, rect.y + rect.height / 3.0), Point::new(rect.x + rect.width, rect.y + rect.height / 3.0));
             let h2 = canvas::Path::line(Point::new(rect.x, rect.y + 2.0 * rect.height / 3.0), Point::new(rect.x + rect.width, rect.y + 2.0 * rect.height / 3.0));
             frame.stroke(&h1, stroke.clone());
             frame.stroke(&h2, stroke);
        } else {
             // No selection, full dim
             let overlay = canvas::Path::rectangle(Point::new(0.0, 0.0), bounds.size());
             frame.fill(&overlay, Color::from_rgba(0.0, 0.0, 0.0, 0.7));
             
             // Text instructions
             frame.fill_text(canvas::Text {
                 content: "Click and drag to crop".to_string(),
                 position: frame.center(),
                 color: Color::WHITE,
                 align_x: iced::alignment::Horizontal::Center.into(),
                 align_y: iced::alignment::Vertical::Center,
                 size: iced::Pixels(20.0),
                 ..Default::default()
             });
        }
        
        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        use iced::mouse;
        
        if let Some(position) = cursor.position_in(bounds) {
            // Normalize position (0.0 to 1.0)
            let norm_x = position.x / bounds.width;
            let norm_y = position.y / bounds.height;
            let norm_p = Point::new(norm_x, norm_y);
            
            match event {
                iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    return Some(canvas::Action::publish(Message::StartCropDrag(norm_p)));
                }
                iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                    return Some(canvas::Action::publish(Message::UpdateCropDrag(norm_p)));
                }
                iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    return Some(canvas::Action::publish(Message::EndCropDrag(norm_p)));
                }
                _ => {}
            }
        }
        None
    }
}

fn rounded_text_input_style(_theme: &iced::Theme, status: iced::widget::text_input::Status) -> iced::widget::text_input::Style {
    let active = iced::widget::text_input::Style {
        background: iced::Background::Color(Color::from_rgba(0.2, 0.2, 0.25, 0.8)),
        border: iced::border::Border {
            radius: 20.0.into(),
            width: 1.0,
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.1),
        },
        icon: Color::from_rgb(0.7, 0.7, 0.7),
        placeholder: Color::from_rgb(0.5, 0.5, 0.5),
        value: Color::WHITE,
        selection: Color::from_rgba(0.5, 0.5, 1.0, 0.3),
    };

    match status {
        iced::widget::text_input::Status::Active => active,
        iced::widget::text_input::Status::Hovered => iced::widget::text_input::Style {
            border: iced::border::Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.3),
                ..active.border
            },
            ..active
        },
        iced::widget::text_input::Status::Focused { .. } => iced::widget::text_input::Style {
             background: iced::Background::Color(Color::from_rgba(0.25, 0.25, 0.3, 0.9)),
             border: iced::border::Border {
                color: Color::from_rgb(0.6, 0.3, 0.9), // Purple highlight
                ..active.border
            },
            ..active
        },
        iced::widget::text_input::Status::Disabled => iced::widget::text_input::Style {
            background: iced::Background::Color(Color::from_rgba(0.1, 0.1, 0.1, 0.5)),
            value: Color::from_rgb(0.5, 0.5, 0.5),
            ..active
        },
    }
}

#[cfg(test)]
mod tests;
