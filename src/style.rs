// Design System: Native Linux Dynamic Island & Control Surface in Zinc-950 Liquid Glass (Borderless)
// Color Palette: Off-black Zinc (#09090b / #121215 / #18181b) + Emerald Accent (#10b981 / #34d399)

pub const APP_CSS: &str = r#"
* {
    font-family: "Inter", "Geist", "Outfit", "Cantarell", sans-serif;
}

window.transparent-window,
window.transparent-window.background,
window.transparent-window.background.csd,
window.transparent-window.background.solid-csd,
.transparent-window,
scrolledwindow,
viewport,
overlay {
    background: transparent;
    background-color: rgba(0, 0, 0, 0);
    background-image: none;
    box-shadow: none;
    border: none;
    outline: none;
}

/* Hide scrollbars inside compact cards unless hovered */
scrollbar {
    background-color: transparent;
    border: none;
}

scrollbar slider {
    background-color: rgba(255, 255, 255, 0.16);
    border-radius: 999px;
    min-width: 4px;
    min-height: 20px;
}

scrollbar slider:hover {
    background-color: rgba(255, 255, 255, 0.30);
}

window.settings-window,
window.settings-window.background,
window.settings-window.background.csd,
.settings-root {
    background-color: #09090b;
    background-image: none;
    color: #f4f4f5;
    border: none;
    box-shadow: none;
}

window.settings-window headerbar {
    background-color: #0c0c0f;
    background-image: none;
    color: #f4f4f5;
    border: none;
    box-shadow: none;
}

.settings-card {
    background-color: #121215;
    background-image: none;
    border: none;
    border-radius: 14px;
    padding: 14px 16px;
    box-shadow: none;
}

.settings-section-title {
    font-family: "JetBrains Mono", "Cantarell", monospace;
    color: #34d399;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.8px;
}

.provider-badge {
    font-family: "JetBrains Mono", "Cantarell", monospace;
    background-color: rgba(16, 185, 129, 0.16);
    color: #34d399;
    border: none;
    border-radius: 999px;
    padding: 2px 8px;
    font-size: 9.5px;
    font-weight: 700;
}

.pill-primary-text {
    color: #f4f4f5;
    font-weight: 600;
    font-size: 12px;
}

.pill-secondary-text {
    color: #d4d4d8;
    font-weight: 500;
    font-size: 11.5px;
}

.lyric-banner-box {
    background-color: rgba(24, 24, 27, 0.85);
    border: none;
    border-radius: 10px;
    padding: 5px 10px;
}

.media-lyric-line {
    color: #34d399;
    font-weight: 600;
    font-size: 11.5px;
}

.pill-icon {
    color: #a1a1aa;
}

.nav-strip {
    background-color: #121215;
    background-image: none;
    border: none;
    border-radius: 999px;
    padding: 2px;
}

button.tab-pill {
    background: transparent;
    background-color: transparent;
    background-image: none;
    color: #a1a1aa;
    border: none;
    border-radius: 999px;
    padding: 4px 8px;
    font-size: 11px;
    font-weight: 600;
    box-shadow: none;
    outline: none;
    transition: background-color 180ms ease, color 160ms ease;
}

button.tab-pill:hover {
    background-color: rgba(255, 255, 255, 0.07);
    color: #f4f4f5;
}

button.tab-pill.active {
    background-color: #f4f4f5;
    color: #09090b;
    padding: 4px 10px;
    font-weight: 700;
}

button.tab-pill.has-unread {
    color: #34d399;
}

button.tab-pill.active.has-unread {
    background-color: #34d399;
    color: #09090b;
}

button.top-icon-btn {
    background: transparent;
    background-color: #121215;
    background-image: none;
    color: #a1a1aa;
    border: none;
    border-radius: 999px;
    min-width: 26px;
    min-height: 26px;
    padding: 4px;
    box-shadow: none;
    transition: background-color 150ms ease, color 150ms ease;
}

button.top-icon-btn:hover {
    background-color: #1f1f23;
    color: #f4f4f5;
}

button.top-icon-btn.pinned {
    background-color: rgba(16, 185, 129, 0.20);
    color: #34d399;
}

.glass-card {
    background-color: #111114;
    background-image: none;
    border: none;
    border-radius: 14px;
    padding: 10px 14px;
    box-shadow: none;
}

.album-frame {
    background-color: #2dd4bf;
    background-image: linear-gradient(135deg, #34d399 0%, #10b981 100%);
    color: #09090b;
    border: none;
    border-radius: 14px;
    min-width: 60px;
    min-height: 60px;
}

.tag-label {
    font-family: "JetBrains Mono", "Cantarell", monospace;
    color: #34d399;
    font-size: 9.5px;
    font-weight: 700;
    letter-spacing: 0.6px;
}

.heading-label {
    color: #f4f4f5;
    font-size: 13.5px;
    font-weight: 700;
    letter-spacing: -0.2px;
}

.sub-label {
    color: #a1a1aa;
    font-size: 11.5px;
    font-weight: 500;
}

.muted-label {
    font-family: "JetBrains Mono", "Cantarell", monospace;
    color: #71717a;
    font-size: 10.5px;
    font-weight: 600;
}

button.media-btn {
    background: none;
    background-color: #18181b;
    background-image: none;
    color: #f4f4f5;
    border: none;
    border-radius: 999px;
    min-width: 32px;
    min-height: 32px;
    padding: 5px;
    box-shadow: none;
    transition: background-color 150ms ease;
}

button.media-btn:hover {
    background-color: #27272a;
}

button.media-btn.play-primary {
    background-color: #f4f4f5;
    color: #09090b;
    min-width: 36px;
    min-height: 36px;
}

button.media-btn.play-primary:hover {
    background-color: #e4e4e7;
}

button.pill-btn {
    background: none;
    background-color: #18181b;
    background-image: none;
    color: #e4e4e7;
    border: none;
    border-radius: 999px;
    padding: 5px 11px;
    font-size: 11px;
    font-weight: 600;
    box-shadow: none;
    transition: background-color 150ms ease, color 150ms ease;
}

button.pill-btn:hover {
    background-color: #27272a;
    color: #f4f4f5;
}

button.pill-btn.primary {
    background-color: #f4f4f5;
    color: #09090b;
    font-weight: 700;
}

button.pill-btn.primary:hover {
    background-color: #e4e4e7;
}

.notif-item-card {
    background-color: #16161a;
    border: none;
    border-radius: 11px;
    padding: 8px 11px;
    transition: background-color 150ms ease;
}

.notif-item-card:hover {
    background-color: #1c1c22;
}

.notif-item-card.unread {
    background-color: rgba(16, 185, 129, 0.08);
}

.notif-item-card.critical {
    background-color: rgba(244, 63, 94, 0.10);
}

.notif-icon-badge {
    background-color: rgba(16, 185, 129, 0.16);
    color: #34d399;
    border: none;
    border-radius: 10px;
    min-width: 30px;
    min-height: 30px;
    padding: 5px;
}

.notif-icon-badge.critical {
    background-color: rgba(244, 63, 94, 0.20);
    color: #fb7185;
}

.notif-icon-badge.sky {
    background-color: rgba(56, 189, 248, 0.18);
    color: #38bdf8;
}

.notif-icon-badge.indigo {
    background-color: rgba(129, 140, 248, 0.18);
    color: #818cf8;
}

.notif-icon-badge.amber {
    background-color: rgba(245, 158, 11, 0.18);
    color: #fbbf24;
}

.notif-urgent-tag {
    font-family: "JetBrains Mono", "Cantarell", monospace;
    background-color: rgba(244, 63, 94, 0.22);
    color: #fda4af;
    border: none;
    border-radius: 999px;
    padding: 1px 6px;
    font-size: 9px;
    font-weight: 700;
}

.list-row-card {
    background-color: #16161a;
    border: none;
    border-radius: 10px;
    padding: 6px 10px;
    transition: background-color 150ms ease;
}

.list-row-card:hover {
    background-color: #1c1c22;
}

.timer-display {
    font-family: "JetBrains Mono", "Cantarell", monospace;
    color: #f4f4f5;
    font-size: 24px;
    font-weight: 700;
    letter-spacing: -0.5px;
}

.dropzone-area {
    background-color: #16161a;
    border: none;
    border-radius: 11px;
    padding: 6px 10px;
}

.sholat-card {
    background-color: #18181b;
    border: none;
    border-radius: 11px;
    padding: 6px 4px;
    transition: background-color 200ms ease;
}

.sholat-card.active {
    background-color: rgba(16, 185, 129, 0.18);
}

.sholat-name {
    color: #a1a1aa;
    font-size: 10px;
    font-weight: 600;
}

.sholat-time {
    font-family: "JetBrains Mono", "Cantarell", monospace;
    color: #f4f4f5;
    font-size: 12px;
    font-weight: 700;
}

entry.island-entry {
    background: none;
    background-color: #18181b;
    background-image: none;
    color: #f4f4f5;
    border: none;
    border-radius: 8px;
    padding: 4px 10px;
    font-size: 11.5px;
    box-shadow: none;
}

/* Dark theme styling for CheckButton & DropDown */
checkbutton {
    color: #e4e4e7;
    font-size: 11.5px;
}

checkbutton check {
    background-color: #18181b;
    background-image: none;
    border: none;
    border-radius: 5px;
    min-width: 15px;
    min-height: 15px;
}

checkbutton check:checked {
    background-color: #10b981;
    color: #09090b;
}

dropdown > button {
    background: none;
    background-color: #18181b;
    background-image: none;
    color: #f4f4f5;
    border: none;
    border-radius: 999px;
    padding: 4px 10px;
    font-size: 11px;
    font-weight: 600;
    box-shadow: none;
}

dropdown > button:hover {
    background-color: #27272a;
}

popover.background > contents {
    background-color: #141418;
    color: #f4f4f5;
    border: none;
    border-radius: 12px;
    padding: 4px;
}

popover listview {
    background-color: transparent;
    color: #f4f4f5;
}

popover listview > row {
    border-radius: 8px;
    padding: 5px 10px;
    color: #e4e4e7;
}

popover listview > row:selected,
popover listview > row:hover {
    background-color: rgba(16, 185, 129, 0.20);
    color: #34d399;
}

progressbar trough {
    background-color: #27272a;
    background-image: none;
    border-radius: 999px;
    min-height: 5px;
}

progressbar progress {
    background-color: #10b981;
    background-image: none;
    border-radius: 999px;
    min-height: 5px;
    transition: all 200ms ease;
}

scale trough {
    background-color: #27272a;
    background-image: none;
    border-radius: 999px;
    min-height: 4px;
}

scale highlight {
    background-color: #10b981;
    background-image: none;
    border-radius: 999px;
}

scale slider {
    background-color: #f4f4f5;
    background-image: none;
    border-radius: 999px;
    min-width: 12px;
    min-height: 12px;
    box-shadow: none;
}
"#;
