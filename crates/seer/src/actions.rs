//! Every command in the app. Each one appears in the menu bar.

use gpui_kit::actions;

actions!(
    seer,
    [
        // App
        Quit,
        Hide,
        HideOthers,
        ShowAll,
        Minimize,
        Zoom,
        // File
        OpenFolder,
        OpenSelection,
        NewFolder,
        Rename,
        RevealInFinder,
        CleanUpStreamNames,
        MoveToTrash,
        // Edit
        SelectAll,
        ClearSelection,
        Find,
        // View
        ToggleSidebar,
        ToggleInspector,
        ToggleHiddenFiles,
        SortByName,
        SortBySize,
        SortByModified,
        // Go
        GoBack,
        GoForward,
        GoToEnclosingFolder,
        GoHome,
        GoDesktop,
        GoDocuments,
        GoDownloads,
        GoMovies,
        GoMusic,
        GoPictures,
        // List navigation
        MoveUp,
        MoveDown,
        ExtendUp,
        ExtendDown,
        MoveToFirst,
        MoveToLast,
        // Inspector
        AnalyzeBitrate,
    ]
);
