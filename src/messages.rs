use iced::widget::image;
use std::path::PathBuf;
use crate::types::*;

#[derive(Debug, Clone)]
pub enum Message {
    // Tab switching
    SwitchTab(Tab),
    
    // Quick Download (Tab 1)
    UrlChanged(String),
    ThumbnailLoaded(Result<image::Handle, String>),
    FormatSelected(DownloadFormat),
    DownloadPressed,
    DownloadProgress(DownloadEvent),
    VideoDurationFetched(Option<f32>),
    ToggleQuickTimeRange(bool),
    UpdateQuickTimeRangeStart(f32),
    UpdateQuickTimeRangeEnd(f32),
    
    // Batch Queue (Tab 2)
    QueueUrlInputChanged(String),
    AddToQueue,
    RemoveQueueItem(usize),
    MoveQueueItemUp(usize),
    MoveQueueItemDown(usize),
    
    // Queue item configuration
    UpdateQueueItemMediaType(usize, MediaType),
    UpdateQueueItemFormat(usize, OutputFormat),
    UpdateQueueItemTimeRangeStart(usize, f32),
    UpdateQueueItemTimeRangeEnd(usize, f32),
    ToggleQueueItemTimeRange(usize, bool),
    
    // Queue item info fetching
    QueueItemInfoFetched(usize, Result<(String, f32), String>),
    QueueItemThumbnailLoaded(usize, Result<image::Handle, String>),
    
    // Batch download
    StartBatchDownload,
    QueueItemDownloadProgress(usize, f32, String),
    QueueItemDownloadComplete(usize, Result<(), String>),
    
    // Shared
    BrowseFolder,
    FolderSelected(Option<PathBuf>),
    Tick(()),
    
    // Advanced
    ToggleAdvanced,
    ToggleEmbedSubs(bool),
    ToggleEmbedThumbnail(bool),
    ToggleRestrictFilenames(bool),
    ProxyChanged(String),
    BrowserSelected(Browser),
    ClearBrowserCookies,
    YouTubeApiKeyChanged(String),
    
    // YouTube Browser (NEW!)
    YouTubeSearchQueryChanged(String),
    YouTubeSearchSubmitted,
    YouTubeVideosLoaded(Vec<YouTubeVideo>),
    YouTubeThumbnailLoaded(String, image::Handle),
    AddYouTubeVideoToQueue(YouTubeVideo),
    LoadTrendingVideos,
    SwitchBrowseMode(BrowseMode),
    
    VideoUrlResolved(Result<(String, f32), String>),
    // Video Player with Timeline Trimming
    OpenVideoPlayer(String, String, f32), // (url, title, duration)
    CloseVideoPlayer,
    // Crop
    ToggleCropMode,
    StartCropDrag(iced::Point),
    UpdateCropDrag(iced::Point),
    EndCropDrag(iced::Point),
    UpdatePlayerPosition(f32),
    SwitchTheme,
    
    // Video Player
    TrimHandlePressed(TrimHandle, f32), // (handle, mouse_x)
    TrimHandleDragged(f32), // mouse_x
    TrimHandleReleased,
    TrimHandleHover(Option<TrimHandle>),
    SeekToPosition(f32),
    AddTrimmedToQueue,
    
    // Dependencies
    DependenciesChecked(bool, String),
    DownloadDependencies,
    DependenciesDownloaded(Result<(), String>),
    
    // UX
    CardHovered(String),
    CardUnhovered,
    EditQueueItem(usize),
    UpdateQueueItem,
}

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Starting,
    Progress(f32, String),
    Finished(Result<(), String>),
}
