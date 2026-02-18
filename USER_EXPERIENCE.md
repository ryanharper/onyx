# Onyx Downloader - User Experience Documentation

Welcome to the comprehensive guide for **Onyx Downloader**. This document details the key features of the application, provides step-by-step instructions for advanced functionality, and serves as a guide for capturing screen recordings to showcase the app.

---

## 📖 Table of Contents
1. [Initial Setup & Dependencies](#1-initial-setup--dependencies)
2. [YouTube API Key Configuration](#2-youtube-api-key-configuration)
3. [Quick Download Guide](#3-quick-download-guide)
4. [Using the YouTube Browser](#4-using-the-youtube-browser)
5. [Batch Queue & Multi-Download](#5-batch-queue--multi-download)
6. [Advanced Video Editing: Trimming & Cropping](#6-advanced-video-editing-trimming--cropping)
7. [Settings & Preferences](#7-settings--preferences)
8. [Screen Recording Guide](#8-screen-recording-guide)

---

## 1. Initial Setup & Dependencies
Onyx is designed to be self-sufficient. On the first launch, the app checks for required core components:
*   **yt-dlp**: The engine for fetching video data.
*   **ffmpeg**: Required for merging video/audio streams and performing trims/crops.

If these are missing, Onyx will present a **Dependency Installation** screen. Simply click **"Download Dependencies"** to let the app automatically fetch and configure the necessary binaries.

---

## 2. YouTube API Key Configuration
To use the built-in YouTube Browser and Search features, you need a Google API Key.

### How to Get a YouTube API Key:
1.  Go to the [Google Cloud Console](https://console.cloud.google.com/).
2.  Create a new project (e.g., "Onyx Downloader").
3.  Navigate to **APIs & Services > Library**.
4.  Search for **"YouTube Data API v3"** and click **Enable**.
5.  Go to **APIs & Services > Credentials**.
6.  Click **Create Credentials > API Key**.
7.  Copy the generated API Key.

### How to Add it in Onyx:
1.  Open Onyx and navigate to the **Settings** tab (gear icon).
2.  Locate the **YouTube API Configuration** section.
3.  Paste your key into the **API Key** field.
4.  The key is saved automatically and will enable the Search and Trending features in the browser.

---

## 3. Quick Download Guide
The **Download** tab is for fast, single-video fetches.

1.  **Paste URL**: Paste a YouTube link into the input field. metadata and a thumbnail will load instantly.
2.  **Select Format**: Choose between "Best Video", "1080p", "720p", "Best Audio", or "MP3".
3.  **Choose Folder**: Click the folder icon to select your destination directory.
4.  **Download**: Click **"START DOWNLOAD"**. An animated progress bar and a "Running Man" animation will show the status.

---

## 4. Using the YouTube Browser
Onyx includes a built-in browser so you don't have to leave the app to find content.

*   **Trending**: The default view shows current trending videos (requires API Key).
*   **Search**: Use the search bar at the top right to find specific videos.
*   **Preview**: Click the "Play" icon on any video card to open it in the built-in player.
*   **Add to Queue**: Click **"Add to Queue"** to prepare the video for download in the Batch tab.

---

## 5. Batch Queue & Multi-Download
The **Queue** tab allows you to manage multiple downloads simultaneously, each with unique settings.

### Managing the Queue:
*   Add videos via the search bar or the YouTube Browser.
*   For each item, you can toggle between **Video** and **Audio** modes.
*   Select specific output formats (MP4, MKV, WEBM for video; MP3, M4A, OPUS, FLAC for audio).

### Individual Customization:
One of Onyx's most powerful features is the ability to customize **each** item in the queue:
1.  Click the **Trim (✂️)** button on a queue item.
2.  This opens the Video Player where you can set a specific **Time Range** and **Crop Area** (see below).
3.  Click **"Save Changes"** to apply these settings only to that specific download.

Click **"DOWNLOAD ALL"** to start the batch process. Onyx will process the items and provide individual progress tracking.

---

## 6. Advanced Video Editing: Trimming & Cropping
Onyx provides professional-grade tools for creating clips.

### Precise Trimming (Time Selection):
*   In the Video Player, use the **Apple-style timeline** below the video.
*   Drag the **yellow handles** to set the start and end points.
*   The selection duration is displayed in real-time.
*   Only the selected portion will be downloaded.

### Visual Cropping:
1.  Click **"Crop Video"** in the player.
2.  A dark overlay with a **Rule of Thirds grid** will appear.
3.  **Click and drag** to select the area you want to keep.
4.  Click **"Done Cropping"** to finalize.
5.  Onyx uses FFmpeg to precisely crop the video to your selection during the download process.

---

## 7. Settings & Preferences
*   **Download Preferences**: Toggle "Embed Subtitles", "Embed Thumbnail", and "ASCII Filenames Only".
*   **Network & Cookies**: Select your primary browser (Chrome, Firefox, etc.) to allow Onyx to use your existing login session. This is essential for downloading Age-Restricted or YouTube Premium content.
*   **Themes**: Switch between "Default" (Blue/Dark) and "Vibrant" (Cyan/Black) modes to match your aesthetic.

---

## 8. Screen Recording Guide
When creating video documentation or tutorials for Onyx, follow these recommended "Scenes":

### Scene A: The First Launch
*   **Action**: Open the app for the first time.
*   **Focus**: Show the Dependency error screen and the smooth "Download Dependencies" process with the animated progress bar.

### Scene B: Setting Up Search
*   **Action**: Go to Settings, paste a YouTube API Key.
*   **Focus**: Show the YouTube Browser transitioning from an empty state to a rich grid of trending videos.

### Scene C: Single Quick Download with Trim
*   **Action**: Paste a URL, select a format, and use the "Download Section" sliders in the main tab.
*   **Focus**: The transition to the "Downloading" state with the stopwatch and running character animations.

### Scene D: The Ultimate Batch Workflow
*   **Action**:
    1.  Add 3 videos from the Browser to the Queue.
    2.  Open the first item, set a **Trim** range.
    3.  Open the second item, draw a **Crop** rectangle (e.g., to create a vertical 9:16 clip).
    4.  Set the third item to **Audio (MP3)**.
    5.  Hit **"DOWNLOAD ALL"**.
*   **Focus**: Emphasize that Onyx handles different tasks (trimming, cropping, converting) all in one batch.

---
*Documentation generated for Onyx Downloader v1.0*
