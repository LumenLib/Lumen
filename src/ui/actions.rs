use gpui::actions;

// 定义全局通用的 Actions
actions!(
    app,
    [
        Quit,
        CloseWindow,
        EmptyTrash,
        ToggleFullscreen,
        // 标准 macOS 菜单所需 action
        HideApp,        // “隐藏 Lumen” (⌘H)
        HideOtherApps,  // “隐藏其他” (⌥⌘H)
        ShowAllApps,    // “全部显示”
        MinimizeWindow, // “最小化” (⌘M)
        ZoomWindow,     // “缩放”
        // 编辑菜单（通过 OsAction 绑定到 macOS 原生 selector）
        Undo,
        Redo,
        Cut,
        Copy,
        Paste,
        SelectAll,
        // 上下文菜单（文献库 / 订阅）所需 action
        AddSourceManual,   // 手动添加文献
        AddSourceBibtex,   // 通过 BibTeX 添加
        AddSourceDoi,      // 通过 DOI 添加
        AddSourceArxiv,    // 通过 ArXiv 添加
        AddSourceDblp,     // 通过 DBLP 添加
        AddSourceOpenalex, // 通过 OpenAlex 添加
        DuplicateSearch,   // 重复文献搜索
        AddSubscription,   // 添加订阅（占位，待改为 dialog 弹窗）
    ]
);
