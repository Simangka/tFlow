
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Action {
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    WordForward,
    WordBackward,
    MoveWordForward,
    MoveWordBackward,
    StartOfLine,
    EndOfLine,
    MoveToStartOfLine,
    MoveToEndOfLine,
    StartOfFile,
    EndOfFile,
    MoveToStartOfFile,
    MoveToEndOfFile,
    PageUp,
    PageDown,
    MovePageUp,
    MovePageDown,
    HalfPageUp,
    HalfPageDown,
    MoveHalfPageUp,
    MoveHalfPageDown,
    MoveToMatchingBrace,
    SmartGotoLine(usize),
    SmartGotoDefinition,

    InsertChar(char),
    InsertNewline,
    InsertTab,
    DeleteBackward,
    DeleteForward,
    DeleteCharForward,
    DeleteCharBackward,
    DeleteWordForward,
    DeleteWordBackward,
    DeleteLine,
    DeleteToEndOfLine,
    JoinLines,
    Indent,
    Unindent,
    IndentLine,
    UnindentLine,
    DuplicateLine,
    MoveLineUp,
    MoveLineDown,
    ToggleComment,

    SwitchToInsertMode,
    SwitchToNormalMode,
    SwitchToVisualMode,
    SwitchToVisualLineMode,
    SwitchToCommandMode,
    SwitchToSearchMode,

    SelectAll,
    SelectLine,
    SelectToMatchingBrace,
    ExpandSelection,
    ShrinkSelection,

    Copy,
    Cut,
    Paste,
    CopyLine,
    CutLine,

    Undo,
    Redo,

    OpenFile,
    OpenFileAt(String),
    SaveFile,
    SaveAs,
    SaveFileAs,
    CloseFile,
    CloseBuffer,
    ReloadFile,
    NewFile,
    GoToLine(Option<usize>),
    JumpToLine,
    FuzzyFindFile,
    FindSymbol,
    FindHeading,
    ScrollUp,
    ScrollDown,

    Find,
    FindNext,
    FindPrevious,
    Replace,
    ReplaceAll,
    SearchForward,
    SearchBackward,
    SearchNext,
    SearchPrevious,
    SearchReplace,
    SearchReplaceAll,
    SearchToggleRegex,
    SearchToggleCaseSensitive,

    SwitchBuffer(usize),
    GoToBuffer(usize),
    NextBuffer,
    PreviousBuffer,
    SplitHorizontal,
    SplitVertical,
    ClosePane,
    CloseSplit,
    NextSplit,
    PreviousSplit,

    FocusPaneLeft,
    FocusPaneRight,
    FocusPaneUp,
    FocusPaneDown,
    FocusEditor,
    FocusPreview,

    ToggleFileTree,
    FocusFileTree,
    ToggleWorkspaceSearch,
    FocusWorkspaceSearch,
    ToggleMarkdownPreview,
    ToggleSideBySide,
    ToggleLineNumbers,
    ToggleRelativeLineNumbers,
    ToggleWordWrap,
    ToggleWrap,
    ToggleSyntaxHighlighting,
    ToggleStatusBar,
    ToggleTabBar,
    ToggleMinimap,
    ToggleLogPanel,

    ShowPalette,
    ShowCommandPalette,
    ShowNotifications,
    IncreaseScrolloff,
    DecreaseScrolloff,
    IncreaseFontSize,
    DecreaseFontSize,
    ResetFontSize,

    ToggleRecording,
    RepeatLastAction,
    MacroStart,
    MacroEnd,
    MacroPlay,
    PlaybackMacro,

    ExecuteCommand(String),

    Quit,
    ForceQuit,
    Suspend,
    DebugInfo,
    ReloadConfig,

    MoveToLine(usize),
    SearchForwardRegex,
    SearchBackwardRegex,

    Noop,
    NoOp,
}

impl Action {
    pub fn description(&self) -> &'static str {
        match self {
            Action::MoveLeft => "Move cursor left",
            Action::MoveRight => "Move cursor right",
            Action::MoveUp => "Move cursor up",
            Action::MoveDown => "Move cursor down",
            Action::WordForward | Action::MoveWordForward => "Move cursor word forward",
            Action::WordBackward | Action::MoveWordBackward => "Move cursor word backward",
            Action::StartOfLine | Action::MoveToStartOfLine => "Move to start of line",
            Action::EndOfLine | Action::MoveToEndOfLine => "Move to end of line",
            Action::StartOfFile | Action::MoveToStartOfFile => "Move to start of file",
            Action::EndOfFile | Action::MoveToEndOfFile => "Move to end of file",
            Action::PageUp | Action::MovePageUp => "Move one page up",
            Action::PageDown | Action::MovePageDown => "Move one page down",
            Action::HalfPageUp | Action::MoveHalfPageUp => "Move half page up",
            Action::HalfPageDown | Action::MoveHalfPageDown => "Move half page down",
            Action::MoveToMatchingBrace => "Move to matching brace",
            Action::SmartGotoLine(_) => "Smart goto line",
            Action::SmartGotoDefinition => "Smart goto definition",
            Action::InsertChar(_) => "Insert character",
            Action::InsertNewline => "Insert newline",
            Action::InsertTab => "Insert tab",
            Action::DeleteBackward | Action::DeleteCharBackward => "Delete character backward",
            Action::DeleteForward | Action::DeleteCharForward => "Delete character forward",
            Action::DeleteWordForward => "Delete word forward",
            Action::DeleteWordBackward => "Delete word backward",
            Action::DeleteLine => "Delete line",
            Action::DeleteToEndOfLine => "Delete to end of line",
            Action::JoinLines => "Join lines",
            Action::Indent | Action::IndentLine => "Indent line",
            Action::Unindent | Action::UnindentLine => "Unindent line",
            Action::DuplicateLine => "Duplicate line",
            Action::MoveLineUp => "Move line up",
            Action::MoveLineDown => "Move line down",
            Action::ToggleComment => "Toggle comment",
            Action::SwitchToInsertMode => "Switch to insert mode",
            Action::SwitchToNormalMode => "Switch to normal mode",
            Action::SwitchToVisualMode => "Switch to visual mode",
            Action::SwitchToVisualLineMode => "Switch to visual line mode",
            Action::SwitchToCommandMode => "Switch to command mode",
            Action::SwitchToSearchMode => "Switch to search mode",
            Action::SelectAll => "Select all",
            Action::SelectLine => "Select line",
            Action::SelectToMatchingBrace => "Select to matching brace",
            Action::ExpandSelection => "Expand selection",
            Action::ShrinkSelection => "Shrink selection",
            Action::Copy => "Copy selection",
            Action::Cut => "Cut selection",
            Action::Paste => "Paste clipboard",
            Action::CopyLine => "Copy line",
            Action::CutLine => "Cut line",
            Action::Undo => "Undo last change",
            Action::Redo => "Redo last change",
            Action::OpenFile => "Open file",
            Action::OpenFileAt(_) => "Open file at path",
            Action::SaveFile => "Save file",
            Action::SaveAs | Action::SaveFileAs => "Save file as",
            Action::CloseFile => "Close file",
            Action::CloseBuffer => "Close buffer",
            Action::ReloadFile => "Reload file",
            Action::NewFile => "New file",
            Action::GoToLine(..) | Action::JumpToLine => "Jump to line",
            Action::FuzzyFindFile => "Fuzzy find file",
            Action::FindSymbol => "Find symbol in document",
            Action::FindHeading => "Find heading in document",
            Action::ScrollUp => "Scroll up",
            Action::ScrollDown => "Scroll down",
            Action::Find => "Find in document",
            Action::FindNext => "Find next match",
            Action::FindPrevious => "Find previous match",
            Action::Replace => "Search and replace",
            Action::ReplaceAll => "Search and replace all",
            Action::SearchForward => "Search forward",
            Action::SearchBackward => "Search backward",
            Action::SearchNext => "Next search match",
            Action::SearchPrevious => "Previous search match",
            Action::SearchReplace => "Search and replace",
            Action::SearchReplaceAll => "Search and replace all",
            Action::SearchToggleRegex => "Toggle regex search",
            Action::SearchToggleCaseSensitive => "Toggle case sensitive search",
            Action::SearchForwardRegex => "Search forward with regex",
            Action::SearchBackwardRegex => "Search backward with regex",
            Action::SwitchBuffer(_) => "Switch to buffer",
            Action::GoToBuffer(_) => "Go to buffer",
            Action::NextBuffer => "Next buffer",
            Action::PreviousBuffer => "Previous buffer",
            Action::SplitHorizontal => "Split horizontally",
            Action::SplitVertical => "Split vertically",
            Action::ClosePane | Action::CloseSplit => "Close split pane",
            Action::NextSplit => "Next split",
            Action::PreviousSplit => "Previous split",
            Action::FocusPaneLeft => "Focus left pane",
            Action::FocusPaneRight => "Focus right pane",
            Action::FocusPaneUp => "Focus pane above",
            Action::FocusPaneDown => "Focus pane below",
            Action::FocusEditor => "Focus editor",
            Action::FocusPreview => "Focus preview",
            Action::FocusFileTree => "Focus file tree",
            Action::FocusWorkspaceSearch => "Focus workspace search",
            Action::ToggleFileTree => "Toggle file tree",
            Action::ToggleWorkspaceSearch => "Toggle workspace search",
            Action::ToggleMarkdownPreview => "Toggle markdown preview",
            Action::ToggleSideBySide => "Toggle side by side preview",
            Action::ToggleLineNumbers => "Toggle line numbers",
            Action::ToggleRelativeLineNumbers => "Toggle relative line numbers",
            Action::ToggleWordWrap | Action::ToggleWrap => "Toggle word wrap",
            Action::ToggleSyntaxHighlighting => "Toggle syntax highlighting",
            Action::ToggleStatusBar => "Toggle status bar",
            Action::ToggleTabBar => "Toggle tab bar",
            Action::ToggleMinimap => "Toggle minimap",
            Action::ToggleLogPanel => "Toggle log panel",
            Action::ShowPalette | Action::ShowCommandPalette => "Show command palette",
            Action::ShowNotifications => "Show notifications",
            Action::IncreaseScrolloff => "Increase scroll offset",
            Action::DecreaseScrolloff => "Decrease scroll offset",
            Action::IncreaseFontSize => "Increase font size",
            Action::DecreaseFontSize => "Decrease font size",
            Action::ResetFontSize => "Reset font size",
            Action::ToggleRecording => "Toggle macro recording",
            Action::RepeatLastAction => "Repeat last action",
            Action::MacroStart => "Start macro recording",
            Action::MacroEnd => "End macro recording",
            Action::MacroPlay | Action::PlaybackMacro => "Play recorded macro",
            Action::ExecuteCommand(_) => "Execute command",
            Action::Quit => "Quit editor",
            Action::ForceQuit => "Force quit without saving",
            Action::Suspend => "Suspend editor",
            Action::DebugInfo => "Show debug information",
            Action::ReloadConfig => "Reload configuration",
            Action::MoveToLine(_) => "Move to specific line",
            Action::Noop | Action::NoOp => "No operation",
        }
    }

    pub fn category(&self) -> ActionCategory {
        match self {
            Action::MoveLeft | Action::MoveRight | Action::MoveUp | Action::MoveDown
            | Action::WordForward | Action::WordBackward
            | Action::MoveWordForward | Action::MoveWordBackward
            | Action::StartOfLine | Action::EndOfLine
            | Action::MoveToStartOfLine | Action::MoveToEndOfLine
            | Action::StartOfFile | Action::EndOfFile
            | Action::MoveToStartOfFile | Action::MoveToEndOfFile
            | Action::PageUp | Action::PageDown
            | Action::MovePageUp | Action::MovePageDown
            | Action::HalfPageUp | Action::HalfPageDown
            | Action::MoveHalfPageUp | Action::MoveHalfPageDown
            | Action::MoveToMatchingBrace | Action::ScrollUp | Action::ScrollDown
            | Action::SmartGotoLine(..) | Action::SmartGotoDefinition => ActionCategory::Movement,

            Action::InsertChar(..) | Action::InsertNewline | Action::InsertTab
            | Action::DeleteBackward | Action::DeleteForward
            | Action::DeleteCharBackward | Action::DeleteCharForward
            | Action::DeleteWordBackward | Action::DeleteWordForward
            | Action::DeleteLine | Action::DeleteToEndOfLine
            | Action::Indent | Action::IndentLine | Action::Unindent | Action::UnindentLine
            | Action::DuplicateLine | Action::MoveLineUp | Action::MoveLineDown
            | Action::JoinLines | Action::ToggleComment
            | Action::Undo | Action::Redo => ActionCategory::Editing,

            Action::SaveFile | Action::SaveAs | Action::SaveFileAs | Action::CloseFile | Action::CloseBuffer
            | Action::OpenFile | Action::OpenFileAt(..) | Action::NewFile
            | Action::ReloadFile | Action::ReloadConfig => ActionCategory::File,

            Action::Cut | Action::CutLine | Action::Copy | Action::CopyLine | Action::Paste => ActionCategory::Clipboard,

            Action::Find | Action::FindNext | Action::FindPrevious
            | Action::SearchForward | Action::SearchBackward
            | Action::SearchNext | Action::SearchPrevious
            | Action::Replace | Action::ReplaceAll
            | Action::SearchReplace | Action::SearchReplaceAll
            | Action::SearchToggleRegex | Action::SearchToggleCaseSensitive
            | Action::SearchForwardRegex | Action::SearchBackwardRegex
            | Action::GoToLine(..) | Action::JumpToLine
            | Action::FuzzyFindFile | Action::FindSymbol | Action::FindHeading => ActionCategory::Search,

            Action::SwitchToInsertMode | Action::SwitchToNormalMode
            | Action::SwitchToVisualMode | Action::SwitchToVisualLineMode
            | Action::SwitchToCommandMode | Action::SwitchToSearchMode => ActionCategory::Mode,

            Action::ShowPalette | Action::ShowCommandPalette | Action::ShowNotifications
            | Action::ToggleFileTree | Action::ToggleMarkdownPreview
            | Action::ToggleLineNumbers | Action::ToggleRelativeLineNumbers
            | Action::ToggleWordWrap | Action::ToggleWrap
            | Action::ToggleSyntaxHighlighting | Action::ToggleStatusBar
            | Action::ToggleSideBySide | Action::ToggleTabBar | Action::ToggleMinimap
            | Action::IncreaseScrolloff | Action::DecreaseScrolloff
            | Action::IncreaseFontSize | Action::DecreaseFontSize | Action::ResetFontSize
            | Action::ToggleLogPanel | Action::ToggleWorkspaceSearch
            | Action::ToggleRecording => ActionCategory::UI,

            Action::FocusPaneLeft | Action::FocusPaneRight | Action::FocusPaneUp | Action::FocusPaneDown
            | Action::FocusEditor | Action::FocusPreview | Action::FocusFileTree
            | Action::FocusWorkspaceSearch
            | Action::SplitHorizontal | Action::SplitVertical
            | Action::ClosePane | Action::CloseSplit | Action::NextSplit | Action::PreviousSplit
            | Action::SwitchBuffer(..) | Action::GoToBuffer(..) | Action::MoveToLine(..)
            | Action::NextBuffer | Action::PreviousBuffer => ActionCategory::Navigation,

            Action::ExecuteCommand(..) | Action::Suspend | Action::DebugInfo => ActionCategory::Application,

            Action::RepeatLastAction | Action::PlaybackMacro
            | Action::MacroStart | Action::MacroEnd | Action::MacroPlay => ActionCategory::Custom,

            Action::SelectAll | Action::SelectLine | Action::SelectToMatchingBrace
            | Action::ExpandSelection | Action::ShrinkSelection => ActionCategory::Selection,

            Action::Noop | Action::NoOp | Action::Quit | Action::ForceQuit => ActionCategory::Application,
        }
    }

    pub fn from_key_code(code: crossterm::event::KeyCode, ctrl: bool, alt: bool) -> Option<Self> {
        use crossterm::event::KeyCode;
        match (code, ctrl, alt) {
            (KeyCode::Char('q'), true, false) => Some(Action::Quit),
            (KeyCode::Char('s'), true, false) => Some(Action::SaveFile),
            (KeyCode::Char('z'), true, false) => Some(Action::Undo),
            (KeyCode::Char('y'), true, false) => Some(Action::Redo),
            (KeyCode::Char('f'), true, false) => Some(Action::Find),
            (KeyCode::Char('p'), true, false) => Some(Action::ShowPalette),
            (KeyCode::Char('n'), true, false) => Some(Action::NewFile),
            (KeyCode::Char('w'), true, false) => Some(Action::CloseFile),
            (KeyCode::Char('c'), true, false) => Some(Action::Copy),
            (KeyCode::Char('x'), true, false) => Some(Action::Cut),
            (KeyCode::Char('v'), true, false) => Some(Action::Paste),
            (KeyCode::Char('a'), true, false) => Some(Action::SelectAll),
            (KeyCode::Char('r'), true, false) => Some(Action::Replace),
            (KeyCode::Up, true, false) => Some(Action::ScrollUp),
            (KeyCode::Down, true, false) => Some(Action::ScrollDown),
            (KeyCode::Tab, false, false) => Some(Action::NextBuffer),
            (KeyCode::BackTab, false, false) => Some(Action::PreviousBuffer),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionCategory {
    Movement,
    Editing,
    Mode,
    Selection,
    Clipboard,
    UndoRedo,
    File,
    Search,
    Navigation,
    Markdown,
    Workspace,
    UI,
    Application,
    Custom,
}
