use ratatui::style::Color;

pub mod syntax;

pub struct Theme {
    pub name: String,
    pub fg: Color,
    pub bg: Color,
    pub cursor: Color,
    pub cursor_line: Color,
    pub selection: Color,
    pub selection_bg: Color,
    pub line_numbers: Color,
    pub line_numbers_bg: Color,
    pub line_numbers_active: Color,
    pub statusline: Color,
    pub statusline_bg: Color,
    pub statusline_fg: Color,
    pub statusline_filename: Color,
    pub statusline_mode: Color,
    pub command_bar: Color,
    pub command_bar_bg: Color,
    pub command_bar_fg: Color,
    pub border: Color,
    pub border_active: Color,
    pub search_highlight: Color,
    pub match_highlight: Color,
    pub current_line: Color,
    pub comment: Color,
    pub keyword: Color,
    pub string: Color,
    pub heading1: Color,
    pub heading2: Color,
    pub heading3: Color,
    pub heading4: Color,
    pub heading5: Color,
    pub heading6: Color,
    pub link: Color,
    pub list: Color,
    pub blockquote: Color,
    pub code_block: Color,
    pub palette: Color,
    pub palette_selection: Color,
    pub tab_active: Color,
    pub tab_inactive: Color,
    pub tab_bg: Color,
    pub notification_info: Color,
    pub notification_error: Color,
    pub notification_success: Color,
    pub notification_warning: Color,
    pub scrollbar: Color,
    pub scrollbar_bg: Color,
    pub syntax_comment: Color,
    pub syntax_keyword: Color,
    pub syntax_type: Color,
    pub syntax_constant: Color,
    pub syntax_number: Color,
    pub syntax_string: Color,
    pub syntax_function: Color,
}

impl Theme {
    pub fn selection_style(&self) -> ratatui::style::Style {
        ratatui::style::Style::default().fg(self.selection).bg(self.selection_bg)
    }

    pub fn cursor_style(&self) -> ratatui::style::Style {
        ratatui::style::Style::default().fg(self.bg).bg(self.cursor)
    }

    pub fn statusline_style(&self) -> ratatui::style::Style {
        ratatui::style::Style::default().fg(self.statusline_fg).bg(self.statusline_bg)
    }

    pub fn line_number_style(&self, active: bool) -> ratatui::style::Style {
        if active {
            ratatui::style::Style::default().fg(self.line_numbers_active)
        } else {
            ratatui::style::Style::default().fg(self.line_numbers)
        }
    }

    pub fn gutter_style(&self) -> ratatui::style::Style {
        ratatui::style::Style::default().fg(self.line_numbers)
    }

    pub fn style(&self, color: Color) -> ratatui::style::Style {
        ratatui::style::Style::default().fg(color)
    }

    pub fn style_bg(&self, fg: Color, bg: Color) -> ratatui::style::Style {
        ratatui::style::Style::default().fg(fg).bg(bg)
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "retro_green" => Self::retro_green(),
            "amber" => Self::amber(),
            "synthwave" => Self::synthwave(),
            "tokyo_night" => Self::tokyo_night(),
            _ => Self::default_dark(),
        }
    }

    fn base_fields(name: &str, fg: Color, bg: Color) -> Self {
        Self {
            name: name.to_string(),
            fg,
            bg,
            cursor: Color::Rgb(255, 200, 0),
            cursor_line: Color::Rgb(40, 40, 40),
            selection: Color::Rgb(60, 60, 120),
            selection_bg: Color::Rgb(60, 60, 120),
            line_numbers: Color::Rgb(120, 120, 120),
            line_numbers_bg: Color::Rgb(25, 25, 25),
            line_numbers_active: Color::Rgb(200, 200, 200),
            statusline: Color::Rgb(200, 200, 200),
            statusline_bg: Color::Rgb(40, 40, 40),
            statusline_fg: Color::Rgb(200, 200, 200),
            statusline_filename: Color::Rgb(255, 200, 100),
            statusline_mode: Color::Rgb(100, 200, 100),
            command_bar: Color::Rgb(180, 180, 180),
            command_bar_bg: Color::Rgb(30, 30, 30),
            command_bar_fg: Color::Rgb(200, 200, 200),
            border: Color::Rgb(80, 80, 80),
            border_active: Color::Rgb(150, 150, 200),
            search_highlight: Color::Rgb(180, 140, 30),
            match_highlight: Color::Rgb(90, 75, 15),
            current_line: Color::Rgb(40, 40, 40),
            comment: Color::Rgb(100, 140, 100),
            keyword: Color::Rgb(86, 156, 214),
            string: Color::Rgb(206, 145, 120),
            heading1: Color::Rgb(255, 160, 60),
            heading2: Color::Rgb(200, 140, 80),
            heading3: Color::Rgb(160, 120, 100),
            heading4: Color::Rgb(140, 120, 120),
            heading5: Color::Rgb(130, 130, 140),
            heading6: Color::Rgb(140, 140, 160),
            link: Color::Rgb(60, 160, 220),
            list: Color::Rgb(160, 200, 120),
            blockquote: Color::Rgb(140, 140, 140),
            code_block: Color::Rgb(100, 100, 100),
            palette: Color::Rgb(50, 50, 70),
            palette_selection: Color::Rgb(70, 70, 120),
            tab_active: Color::Rgb(200, 200, 200),
            tab_inactive: Color::Rgb(100, 100, 100),
            tab_bg: Color::Rgb(35, 35, 45),
            notification_info: Color::Rgb(60, 140, 200),
            notification_error: Color::Rgb(200, 60, 60),
            notification_success: Color::Rgb(60, 180, 80),
            notification_warning: Color::Rgb(200, 180, 60),
            scrollbar: Color::Rgb(100, 100, 100),
            scrollbar_bg: Color::Rgb(25, 25, 25),
            syntax_comment: Color::Rgb(100, 140, 100),
            syntax_keyword: Color::Rgb(86, 156, 214),
            syntax_type: Color::Rgb(78, 201, 176),
            syntax_constant: Color::Rgb(220, 120, 180),
            syntax_number: Color::Rgb(181, 206, 168),
            syntax_string: Color::Rgb(206, 145, 120),
            syntax_function: Color::Rgb(220, 220, 170),
        }
    }

    pub fn default_dark() -> Self {
        Self::base_fields("default_dark", Color::Rgb(212, 212, 212), Color::Rgb(30, 30, 30))
    }

    pub fn retro_green() -> Self {
        let mut t = Self::base_fields("retro_green", Color::Rgb(51, 255, 51), Color::Rgb(0, 20, 0));
        t.cursor = Color::Rgb(255, 255, 255);
        t.selection = Color::Rgb(0, 60, 0);
        t.selection_bg = Color::Rgb(0, 60, 0);
        t.line_numbers = Color::Rgb(0, 100, 0);
        t.line_numbers_bg = Color::Rgb(0, 15, 0);
        t.line_numbers_active = Color::Rgb(51, 255, 51);
        t.statusline = Color::Rgb(51, 200, 51);
        t.statusline_bg = Color::Rgb(0, 30, 0);
        t.statusline_fg = Color::Rgb(51, 200, 51);
        t.statusline_filename = Color::Rgb(51, 255, 51);
        t.statusline_mode = Color::Rgb(51, 200, 51);
        t.command_bar = Color::Rgb(51, 180, 51);
        t.command_bar_bg = Color::Rgb(0, 15, 0);
        t.command_bar_fg = Color::Rgb(51, 180, 51);
        t.border = Color::Rgb(0, 60, 0);
        t.border_active = Color::Rgb(0, 120, 0);
        t.search_highlight = Color::Rgb(180, 140, 30);
        t.match_highlight = Color::Rgb(70, 55, 10);
        t.current_line = Color::Rgb(0, 35, 0);
        t.comment = Color::Rgb(0, 100, 0);
        t.keyword = Color::Rgb(51, 255, 51);
        t.string = Color::Rgb(102, 255, 102);
        t.heading1 = Color::Rgb(255, 200, 0);
        t.heading2 = Color::Rgb(200, 160, 0);
        t.heading3 = Color::Rgb(150, 120, 0);
        t.heading4 = Color::Rgb(120, 100, 0);
        t.heading5 = Color::Rgb(100, 80, 0);
        t.heading6 = Color::Rgb(80, 70, 0);
        t.link = Color::Rgb(0, 200, 255);
        t.list = Color::Rgb(51, 255, 51);
        t.blockquote = Color::Rgb(0, 120, 0);
        t.code_block = Color::Rgb(0, 80, 0);
        t.palette = Color::Rgb(0, 40, 0);
        t.palette_selection = Color::Rgb(0, 70, 0);
        t.tab_active = Color::Rgb(51, 200, 51);
        t.tab_inactive = Color::Rgb(0, 80, 0);
        t.tab_bg = Color::Rgb(0, 25, 0);
        t.notification_info = Color::Rgb(0, 150, 200);
        t.notification_error = Color::Rgb(255, 51, 51);
        t.notification_success = Color::Rgb(51, 255, 51);
        t.notification_warning = Color::Rgb(200, 180, 0);
        t.scrollbar = Color::Rgb(0, 80, 0);
        t.scrollbar_bg = Color::Rgb(0, 15, 0);
        t.syntax_comment = Color::Rgb(0, 100, 0);
        t.syntax_keyword = Color::Rgb(51, 255, 51);
        t.syntax_type = Color::Rgb(102, 255, 102);
        t.syntax_constant = Color::Rgb(255, 200, 0);
        t.syntax_number = Color::Rgb(102, 255, 102);
        t.syntax_string = Color::Rgb(200, 255, 200);
        t.syntax_function = Color::Rgb(100, 200, 255);
        t
    }

    pub fn amber() -> Self {
        let mut t = Self::base_fields("amber", Color::Rgb(255, 176, 0), Color::Rgb(10, 10, 0));
        t.cursor = Color::Rgb(255, 220, 100);
        t.selection = Color::Rgb(60, 40, 0);
        t.selection_bg = Color::Rgb(60, 40, 0);
        t.line_numbers = Color::Rgb(100, 70, 0);
        t.line_numbers_bg = Color::Rgb(8, 8, 0);
        t.line_numbers_active = Color::Rgb(255, 176, 0);
        t.statusline = Color::Rgb(200, 140, 0);
        t.statusline_bg = Color::Rgb(20, 15, 0);
        t.statusline_fg = Color::Rgb(200, 140, 0);
        t.statusline_filename = Color::Rgb(255, 200, 100);
        t.statusline_mode = Color::Rgb(200, 140, 0);
        t.command_bar = Color::Rgb(180, 120, 0);
        t.command_bar_bg = Color::Rgb(5, 5, 0);
        t.command_bar_fg = Color::Rgb(180, 120, 0);
        t.border = Color::Rgb(60, 40, 0);
        t.border_active = Color::Rgb(120, 80, 0);
        t.current_line = Color::Rgb(20, 15, 0);
        t.comment = Color::Rgb(80, 60, 0);
        t.keyword = Color::Rgb(255, 200, 50);
        t.string = Color::Rgb(200, 150, 50);
        t.heading1 = Color::Rgb(255, 200, 50);
        t.heading2 = Color::Rgb(200, 160, 40);
        t.heading3 = Color::Rgb(160, 130, 30);
        t.heading4 = Color::Rgb(130, 100, 20);
        t.heading5 = Color::Rgb(100, 80, 15);
        t.heading6 = Color::Rgb(80, 60, 10);
        t.link = Color::Rgb(100, 200, 255);
        t.list = Color::Rgb(255, 176, 0);
        t.blockquote = Color::Rgb(120, 90, 0);
        t.code_block = Color::Rgb(60, 40, 0);
        t.palette = Color::Rgb(30, 20, 0);
        t.palette_selection = Color::Rgb(50, 35, 0);
        t.tab_active = Color::Rgb(200, 140, 0);
        t.tab_inactive = Color::Rgb(80, 60, 0);
        t.tab_bg = Color::Rgb(15, 10, 0);
        t.syntax_comment = Color::Rgb(80, 60, 0);
        t.syntax_keyword = Color::Rgb(255, 200, 50);
        t.syntax_type = Color::Rgb(255, 160, 50);
        t.syntax_constant = Color::Rgb(200, 100, 0);
        t.syntax_number = Color::Rgb(200, 150, 50);
        t.syntax_string = Color::Rgb(200, 150, 50);
        t.syntax_function = Color::Rgb(255, 200, 100);
        t
    }

    pub fn synthwave() -> Self {
        let mut t = Self::base_fields("synthwave", Color::Rgb(220, 200, 255), Color::Rgb(20, 20, 40));
        t.cursor = Color::Rgb(255, 100, 200);
        t.selection = Color::Rgb(80, 40, 100);
        t.selection_bg = Color::Rgb(80, 40, 100);
        t.line_numbers = Color::Rgb(100, 80, 120);
        t.line_numbers_bg = Color::Rgb(16, 16, 36);
        t.line_numbers_active = Color::Rgb(220, 200, 255);
        t.statusline = Color::Rgb(200, 180, 255);
        t.statusline_bg = Color::Rgb(30, 25, 50);
        t.statusline_fg = Color::Rgb(200, 180, 255);
        t.statusline_filename = Color::Rgb(255, 200, 100);
        t.statusline_mode = Color::Rgb(200, 100, 200);
        t.command_bar = Color::Rgb(180, 160, 220);
        t.command_bar_bg = Color::Rgb(15, 15, 35);
        t.command_bar_fg = Color::Rgb(180, 160, 220);
        t.border = Color::Rgb(80, 60, 100);
        t.border_active = Color::Rgb(150, 100, 200);
        t.current_line = Color::Rgb(30, 28, 50);
        t.comment = Color::Rgb(100, 80, 120);
        t.keyword = Color::Rgb(255, 80, 200);
        t.string = Color::Rgb(255, 200, 100);
        t.heading1 = Color::Rgb(255, 100, 200);
        t.heading2 = Color::Rgb(200, 80, 160);
        t.heading3 = Color::Rgb(160, 60, 130);
        t.heading4 = Color::Rgb(130, 50, 110);
        t.heading5 = Color::Rgb(110, 40, 90);
        t.heading6 = Color::Rgb(90, 30, 80);
        t.link = Color::Rgb(80, 200, 255);
        t.list = Color::Rgb(200, 120, 255);
        t.blockquote = Color::Rgb(120, 100, 140);
        t.code_block = Color::Rgb(60, 50, 80);
        t.palette = Color::Rgb(40, 30, 60);
        t.palette_selection = Color::Rgb(70, 50, 100);
        t.tab_active = Color::Rgb(200, 180, 255);
        t.tab_inactive = Color::Rgb(80, 60, 100);
        t.tab_bg = Color::Rgb(25, 20, 40);
        t.syntax_comment = Color::Rgb(100, 80, 120);
        t.syntax_keyword = Color::Rgb(255, 80, 200);
        t.syntax_type = Color::Rgb(150, 120, 255);
        t.syntax_constant = Color::Rgb(255, 100, 200);
        t.syntax_number = Color::Rgb(255, 180, 100);
        t.syntax_string = Color::Rgb(255, 200, 100);
        t.syntax_function = Color::Rgb(80, 200, 255);
        t
    }

    pub fn tokyo_night() -> Self {
        let mut t = Self::base_fields("tokyo_night", Color::Rgb(169, 177, 214), Color::Rgb(26, 27, 38));
        t.cursor = Color::Rgb(192, 202, 245);
        t.selection = Color::Rgb(55, 59, 85);
        t.selection_bg = Color::Rgb(55, 59, 85);
        t.line_numbers = Color::Rgb(70, 74, 100);
        t.line_numbers_bg = Color::Rgb(22, 23, 33);
        t.line_numbers_active = Color::Rgb(169, 177, 214);
        t.statusline = Color::Rgb(158, 206, 186);
        t.statusline_bg = Color::Rgb(36, 38, 54);
        t.statusline_fg = Color::Rgb(158, 206, 186);
        t.statusline_filename = Color::Rgb(224, 175, 104);
        t.statusline_mode = Color::Rgb(137, 130, 208);
        t.command_bar = Color::Rgb(150, 160, 200);
        t.command_bar_bg = Color::Rgb(22, 23, 33);
        t.command_bar_fg = Color::Rgb(150, 160, 200);
        t.border = Color::Rgb(55, 59, 85);
        t.border_active = Color::Rgb(97, 175, 239);
        t.current_line = Color::Rgb(36, 38, 54);
        t.comment = Color::Rgb(86, 91, 118);
        t.keyword = Color::Rgb(137, 130, 208);
        t.string = Color::Rgb(158, 206, 106);
        t.heading1 = Color::Rgb(247, 118, 142);
        t.heading2 = Color::Rgb(224, 175, 104);
        t.heading3 = Color::Rgb(158, 206, 106);
        t.heading4 = Color::Rgb(122, 162, 90);
        t.heading5 = Color::Rgb(100, 130, 80);
        t.heading6 = Color::Rgb(86, 91, 118);
        t.link = Color::Rgb(97, 175, 239);
        t.list = Color::Rgb(137, 130, 208);
        t.blockquote = Color::Rgb(100, 106, 140);
        t.code_block = Color::Rgb(66, 70, 98);
        t.palette = Color::Rgb(36, 38, 54);
        t.palette_selection = Color::Rgb(55, 59, 85);
        t.tab_active = Color::Rgb(169, 177, 214);
        t.tab_inactive = Color::Rgb(70, 74, 100);
        t.tab_bg = Color::Rgb(30, 32, 46);
        t.syntax_comment = Color::Rgb(86, 91, 118);
        t.syntax_keyword = Color::Rgb(137, 130, 208);
        t.syntax_type = Color::Rgb(78, 201, 176);
        t.syntax_constant = Color::Rgb(247, 118, 142);
        t.syntax_number = Color::Rgb(158, 206, 106);
        t.syntax_string = Color::Rgb(158, 206, 106);
        t.syntax_function = Color::Rgb(97, 175, 239);
        t
    }
}
