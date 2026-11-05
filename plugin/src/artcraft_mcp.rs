// ArtCraft Suite — MCA / MCP Command Executor for WordApp

impl WordApp {
    fn ensure_active_document_for_agent(&mut self) {
        if !self.has_document() || self.ui.backstage {
            self.no_document = None;
            self.ui.backstage = false;
        }
    }

    fn normalize_wordcraft_cmd(cmd: &str) -> &str {
        match cmd {
            "format.heading1" | "style.heading1" | "heading1" => "para.heading1",
            "format.heading2" | "style.heading2" | "heading2" => "para.heading2",
            "format.heading3" | "style.heading3" | "heading3" => "para.heading3",
            "format.normal" | "style.normal" | "normal" => "para.normal",
            "list.bullet" | "format.bullets" | "bullets" => "para.bullets",
            "list.number" | "format.numbering" | "numbering" => "para.numbering",
            "format.fontSize" | "font.size" => "format.size",
            "format.fontColor" | "font.color" => "format.color",
            "paragraph.alignLeft" | "align.left" => "para.alignLeft",
            "paragraph.alignCenter" | "align.center" => "para.alignCenter",
            "paragraph.alignRight" | "align.right" => "para.alignRight",
            "paragraph.alignJustify" | "align.justify" => "para.justify",
            "edit.selectAll" => "select.all",
            "edit.delete" => "text.delete",
            other => other,
        }
    }

    /// Inserts inline text while honoring `**bold**` spans cleanly.
    fn insert_inline_formatted_text(&mut self, text: &str) {
        let mut rest = text;
        while let Some(start) = rest.find("**") {
            let before = &rest[..start];
            if !before.is_empty() {
                let _ = self.run("text.insert", serde_json::json!({"text": before}));
            }
            let after_start = &rest[start + 2..];
            if let Some(end) = after_start.find("**") {
                let bold_part = &after_start[..end];
                if !bold_part.is_empty() {
                    let _ = self.run("format.bold", serde_json::json!({"value": true}));
                    let _ = self.run("text.insert", serde_json::json!({"text": bold_part}));
                    let _ = self.run("format.bold", serde_json::json!({"value": false}));
                }
                rest = &after_start[end + 2..];
            } else {
                let _ = self.run("text.insert", serde_json::json!({"text": &rest[start..]}));
                return;
            }
        }
        if !rest.is_empty() {
            let _ = self.run("text.insert", serde_json::json!({"text": rest}));
        }
    }

    /// Inserts a styled multi-column table cell-by-cell using `insert.table` + `text.tab`,
    /// never dumping text into cell (0,0) and cleanly exiting the table afterwards.
    fn insert_styled_table_helper(&mut self, spec: &serde_json::Value) -> Result<serde_json::Value, String> {
        self.ensure_active_document_for_agent();
        let _ = self.run("caret.docEnd", serde_json::json!({}));
        let body_now = self.session.doc.plain_text(wordcraft_doc::StoryRef::Body);
        if !body_now.trim().is_empty() && !body_now.ends_with('\n') {
            let _ = self.run("text.newParagraph", serde_json::json!({}));
        }
        let _ = self.run("para.normal", serde_json::json!({}));
        let _ = self.run("format.bold", serde_json::json!({"value": false}));
        let _ = self.run("format.color", serde_json::json!({"color": "auto"}));

        if let Some(title) = spec.get("title").and_then(serde_json::Value::as_str) {
            if !title.trim().is_empty() {
                let _ = self.run("para.heading2", serde_json::json!({}));
                let _ = self.run("text.insert", serde_json::json!({"text": title.trim()}));
                let _ = self.run("text.newParagraph", serde_json::json!({}));
                let _ = self.run("para.normal", serde_json::json!({}));
            }
        }

        let headers: Vec<String> = spec
            .get("headers")
            .or_else(|| spec.get("columns"))
            .and_then(serde_json::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string().trim_matches('"').to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let rows: Vec<Vec<String>> = spec
            .get("rows")
            .or_else(|| spec.get("data"))
            .or_else(|| spec.get("values"))
            .and_then(serde_json::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        r.as_array().map(|cells| {
                            cells
                                .iter()
                                .map(|c| c.as_str().map(str::to_string).unwrap_or_else(|| c.to_string().trim_matches('"').to_string()))
                                .collect()
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let cols = headers
            .len()
            .max(rows.iter().map(Vec::len).max().unwrap_or(0))
            .max(1);
        let has_header = !headers.is_empty();
        let total_rows = (if has_header { 1 } else { 0 } + rows.len()).max(1);

        let header_bg = spec
            .get("headerBg")
            .or_else(|| spec.get("headerColor"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("1E293B")
            .trim_start_matches('#');
        let header_fg = spec
            .get("headerTextColor")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("FFFFFF")
            .trim_start_matches('#');
        let zebra = spec.get("zebraStripes").and_then(serde_json::Value::as_bool).unwrap_or(true);
        let zebra_bg = spec
            .get("zebraColor")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("F1F5F9")
            .trim_start_matches('#');

        self.run("insert.table", serde_json::json!({"rows": total_rows, "cols": cols}))?;
        let _ = self.run("table.look", serde_json::json!({"headerRow": has_header, "bandedRows": zebra}));
        let _ = self.run("table.borders", serde_json::json!({"kind": "all", "width": 0.75, "color": "CBD5E1"}));

        let total_cells = total_rows * cols;
        let mut cell_idx = 0usize;

        if has_header {
            for c in 0..cols {
                let text = headers.get(c).map(String::as_str).unwrap_or("");
                let _ = self.run("table.shading", serde_json::json!({"color": header_bg}));
                let _ = self.run("format.bold", serde_json::json!({"value": true}));
                let _ = self.run("format.color", serde_json::json!({"color": header_fg}));
                let clean = text.replace("**", "");
                if !clean.is_empty() {
                    let _ = self.run("text.insert", serde_json::json!({"text": clean}));
                }
                let _ = self.run("format.bold", serde_json::json!({"value": false}));
                let _ = self.run("format.color", serde_json::json!({"color": "auto"}));
                cell_idx += 1;
                if cell_idx < total_cells {
                    let _ = self.run("text.tab", serde_json::json!({}));
                }
            }
        }

        for (r_idx, row) in rows.iter().enumerate() {
            let row_bg = if zebra && r_idx % 2 == 1 { zebra_bg } else { "FFFFFF" };
            for c in 0..cols {
                let text = row.get(c).map(String::as_str).unwrap_or("");
                let _ = self.run("table.shading", serde_json::json!({"color": row_bg}));
                let _ = self.run("format.bold", serde_json::json!({"value": false}));
                let _ = self.run("format.color", serde_json::json!({"color": "0F172A"}));
                if !text.is_empty() {
                    self.insert_inline_formatted_text(text);
                }
                let _ = self.run("format.color", serde_json::json!({"color": "auto"}));
                cell_idx += 1;
                if cell_idx < total_cells {
                    let _ = self.run("text.tab", serde_json::json!({}));
                }
            }
        }

        // Move caret cleanly out of the table to the trailing paragraph below it
        let _ = self.run("caret.docEnd", serde_json::json!({}));
        let _ = self.run("para.normal", serde_json::json!({}));
        let _ = self.run("format.bold", serde_json::json!({"value": false}));
        let _ = self.run("format.color", serde_json::json!({"color": "auto"}));

        Ok(serde_json::json!({
            "status": "ok",
            "rows": total_rows,
            "cols": cols
        }))
    }

    /// Parses rich Markdown (Headings, Callouts, Bullet/Numbered Lists, Bold/Italic, and Markdown Tables)
    /// and inserts them as native formatted WordCraft blocks and tables.
    fn insert_markdown_into_wordcraft(&mut self, md: &str, move_to_end: bool) -> Result<serde_json::Value, String> {
        self.ensure_active_document_for_agent();
        if move_to_end {
            let _ = self.run("caret.docEnd", serde_json::json!({}));
            let existing = self.session.doc.plain_text(wordcraft_doc::StoryRef::Body);
            if !existing.trim().is_empty() {
                let _ = self.run("text.newParagraph", serde_json::json!({}));
            }
        }

        let lines: Vec<&str> = md.lines().collect();
        let mut i = 0usize;
        while i < lines.len() {
            let trimmed = lines[i].trim();
            if trimmed.is_empty() {
                i += 1;
                continue;
            }
            if trimmed.starts_with("```") {
                i += 1;
                while i < lines.len() && !lines[i].trim().starts_with("```") {
                    i += 1;
                }
                i += 1;
                continue;
            }
            // Markdown Table block: | A | B |
            if trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() > 2 {
                let mut table_lines = Vec::new();
                while i < lines.len() {
                    let tl = lines[i].trim();
                    if tl.starts_with('|') && tl.ends_with('|') {
                        table_lines.push(tl);
                        i += 1;
                    } else {
                        break;
                    }
                }
                let mut parsed_rows: Vec<Vec<String>> = Vec::new();
                for tl in table_lines {
                    let inner = &tl[1..tl.len() - 1];
                    let cells: Vec<String> = inner.split('|').map(|c| c.trim().to_string()).collect();
                    let is_sep = cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':' | ' ')));
                    if !is_sep {
                        parsed_rows.push(cells);
                    }
                }
                if !parsed_rows.is_empty() {
                    let headers = parsed_rows.remove(0);
                    let _ = self.insert_styled_table_helper(&serde_json::json!({
                        "headers": headers,
                        "rows": parsed_rows,
                        "zebraStripes": true
                    }));
                }
                continue;
            }

            if trimmed == "---" || trimmed == "***" {
                let _ = self.run("para.normal", serde_json::json!({}));
                let _ = self.run("para.borders", serde_json::json!({"kind": "bottom", "width": 1.0, "color": "CBD5E1"}));
                let _ = self.run("text.newParagraph", serde_json::json!({}));
                let _ = self.run("para.borders", serde_json::json!({"kind": "none"}));
                i += 1;
                continue;
            }

            if let Some(h3) = trimmed.strip_prefix("### ") {
                let _ = self.run("para.heading3", serde_json::json!({}));
                self.insert_inline_formatted_text(h3.trim());
                let _ = self.run("text.newParagraph", serde_json::json!({}));
                let _ = self.run("para.normal", serde_json::json!({}));
            } else if let Some(h2) = trimmed.strip_prefix("## ") {
                let _ = self.run("para.heading2", serde_json::json!({}));
                self.insert_inline_formatted_text(h2.trim());
                let _ = self.run("text.newParagraph", serde_json::json!({}));
                let _ = self.run("para.normal", serde_json::json!({}));
            } else if let Some(h1) = trimmed.strip_prefix("# ") {
                let _ = self.run("para.heading1", serde_json::json!({}));
                self.insert_inline_formatted_text(h1.trim());
                let _ = self.run("text.newParagraph", serde_json::json!({}));
                let _ = self.run("para.normal", serde_json::json!({}));
            } else if let Some(quote) = trimmed.strip_prefix("> ") {
                let _ = self.run("para.normal", serde_json::json!({}));
                let _ = self.run("para.shading", serde_json::json!({"color": "EFF6FF"}));
                let _ = self.run("para.borders", serde_json::json!({"kind": "left", "width": 3.0, "color": "3B82F6"}));
                let _ = self.run("para.indents", serde_json::json!({"left": 12.0, "right": 12.0}));
                let _ = self.run("para.spacing", serde_json::json!({"before": 6.0, "after": 8.0}));
                self.insert_inline_formatted_text(quote.trim());
                let _ = self.run("text.newParagraph", serde_json::json!({}));
                let _ = self.run("para.shading", serde_json::json!({"color": null}));
                let _ = self.run("para.borders", serde_json::json!({"kind": "none"}));
                let _ = self.run("para.indents", serde_json::json!({"left": 0.0, "right": 0.0}));
            } else if let Some(bullet) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")).or_else(|| trimmed.strip_prefix("• ")) {
                let _ = self.run("para.bullets", serde_json::json!({}));
                self.insert_inline_formatted_text(bullet.trim());
                let _ = self.run("text.newParagraph", serde_json::json!({}));
                let _ = self.run("para.bullets", serde_json::json!({"off": true}));
            } else if trimmed.chars().next().is_some_and(|c| c.is_ascii_digit()) && trimmed.contains(". ") {
                if let Some((num_part, rest_item)) = trimmed.split_once(". ") {
                    if num_part.chars().all(|c| c.is_ascii_digit()) {
                        let _ = self.run("para.numbering", serde_json::json!({}));
                        self.insert_inline_formatted_text(rest_item.trim());
                        let _ = self.run("text.newParagraph", serde_json::json!({}));
                        let _ = self.run("para.numbering", serde_json::json!({"off": true}));
                        i += 1;
                        continue;
                    }
                }
                let _ = self.run("para.normal", serde_json::json!({}));
                self.insert_inline_formatted_text(trimmed);
                let _ = self.run("text.newParagraph", serde_json::json!({}));
            } else {
                let _ = self.run("para.normal", serde_json::json!({}));
                self.insert_inline_formatted_text(trimmed);
                let _ = self.run("text.newParagraph", serde_json::json!({}));
            }
            i += 1;
        }

        Ok(serde_json::json!({"status": "ok"}))
    }

    fn execute_rich_layout_spec(&mut self, params: &serde_json::Value) -> Result<serde_json::Value, String> {
        self.ensure_active_document_for_agent();
        if params.get("clear").and_then(serde_json::Value::as_bool) == Some(true) {
            let _ = self.run("select.all", serde_json::json!({}));
            let _ = self.run("text.delete", serde_json::json!({}));
        }

        if let Some(title) = params.get("title").and_then(serde_json::Value::as_str) {
            if !title.trim().is_empty() {
                let _ = self.run("caret.docEnd", serde_json::json!({}));
                let _ = self.run("para.heading1", serde_json::json!({}));
                let _ = self.run("text.insert", serde_json::json!({"text": title.trim()}));
                let _ = self.run("text.newParagraph", serde_json::json!({}));
                let _ = self.run("para.normal", serde_json::json!({}));
            }
        }
        if let Some(sub) = params.get("subtitle").and_then(serde_json::Value::as_str) {
            if !sub.trim().is_empty() {
                let _ = self.run("format.italic", serde_json::json!({"value": true}));
                let _ = self.run("format.color", serde_json::json!({"color": "475569"}));
                let _ = self.run("text.insert", serde_json::json!({"text": sub.trim()}));
                let _ = self.run("format.italic", serde_json::json!({"value": false}));
                let _ = self.run("format.color", serde_json::json!({"color": "auto"}));
                let _ = self.run("text.newParagraph", serde_json::json!({}));
            }
        }

        if let Some(sections) = params.get("sections").or_else(|| params.get("blocks")).and_then(serde_json::Value::as_array) {
            for sec in sections {
                if let Some(h) = sec.get("heading").or_else(|| sec.get("title")).and_then(serde_json::Value::as_str) {
                    let lvl = sec.get("level").and_then(serde_json::Value::as_u64).unwrap_or(2);
                    let style_cmd = match lvl {
                        1 => "para.heading1",
                        3 => "para.heading3",
                        _ => "para.heading2",
                    };
                    let _ = self.run(style_cmd, serde_json::json!({}));
                    self.insert_inline_formatted_text(h.trim());
                    let _ = self.run("text.newParagraph", serde_json::json!({}));
                    let _ = self.run("para.normal", serde_json::json!({}));
                }
                if let Some(callout) = sec.get("callout").or_else(|| sec.get("highlight")).and_then(serde_json::Value::as_str) {
                    let _ = self.insert_markdown_into_wordcraft(&format!("> {callout}"), false);
                }
                if let Some(p_str) = sec.get("text").or_else(|| sec.get("content")).or_else(|| sec.get("markdown")).and_then(serde_json::Value::as_str) {
                    let _ = self.insert_markdown_into_wordcraft(p_str, false);
                }
                if let Some(paras) = sec.get("paragraphs").and_then(serde_json::Value::as_array) {
                    for p in paras.iter().filter_map(serde_json::Value::as_str) {
                        let _ = self.insert_markdown_into_wordcraft(p, false);
                    }
                }
                if let Some(bullets) = sec.get("bullets").or_else(|| sec.get("items")).and_then(serde_json::Value::as_array) {
                    for b in bullets.iter().filter_map(serde_json::Value::as_str) {
                        let _ = self.run("para.bullets", serde_json::json!({}));
                        self.insert_inline_formatted_text(b.trim());
                        let _ = self.run("text.newParagraph", serde_json::json!({}));
                        let _ = self.run("para.bullets", serde_json::json!({"off": true}));
                    }
                }
                if let Some(tbl) = sec.get("table") {
                    let _ = self.insert_styled_table_helper(tbl);
                }
            }
        }

        if let Some(md) = params.get("markdown").or_else(|| params.get("content")).or_else(|| params.get("text")).and_then(serde_json::Value::as_str) {
            let _ = self.insert_markdown_into_wordcraft(md, true);
        }

        if let Some(tbl) = params.get("table") {
            let _ = self.insert_styled_table_helper(tbl);
        }

        Ok(serde_json::json!({"status": "ok"}))
    }

    /// Polls and executes AI Agent tool calls on the main thread.
    pub fn poll_artcraft_agent(&mut self) {
        let reqs = self.artcraft_agent.take_tool_requests();
        for req in reqs {
            let res = match req.tool_name.as_str() {
                "doc.insertStyledTable" | "insert_styled_table" | "table.insertStyled" => {
                    self.insert_styled_table_helper(&req.params)
                }
                "doc.insertRichLayout" | "doc.generateFancyDocument" | "insert_markdown_from_chat" => {
                    self.execute_rich_layout_spec(&req.params)
                }
                "doc.clearAndWrite" => {
                    self.ensure_active_document_for_agent();
                    let _ = self.run("select.all", serde_json::json!({}));
                    let _ = self.run("text.delete", serde_json::json!({}));
                    self.execute_rich_layout_spec(&req.params)
                }
                "doc.replaceSelection" | "replace_selection_from_chat" => {
                    self.ensure_active_document_for_agent();
                    if !self.session.sel.is_collapsed() {
                        let _ = self.run("text.delete", serde_json::json!({}));
                    }
                    let md = req.params.get("markdown").or_else(|| req.params.get("text")).or_else(|| req.params.get("content")).and_then(serde_json::Value::as_str).unwrap_or("");
                    self.insert_markdown_into_wordcraft(md, false)
                }
                "doc.formatSelection" => {
                    self.ensure_active_document_for_agent();
                    if let Some(st) = req.params.get("style").and_then(serde_json::Value::as_str) {
                        let norm = Self::normalize_wordcraft_cmd(st);
                        if norm.starts_with("para.") {
                            let _ = self.run(norm, serde_json::json!({}));
                        } else {
                            let _ = self.run("para.style", serde_json::json!({"style": st}));
                        }
                    }
                    if let Some(b) = req.params.get("bold").and_then(serde_json::Value::as_bool) {
                        let _ = self.run("format.bold", serde_json::json!({"value": b}));
                    }
                    if let Some(it) = req.params.get("italic").and_then(serde_json::Value::as_bool) {
                        let _ = self.run("format.italic", serde_json::json!({"value": it}));
                    }
                    if let Some(u) = req.params.get("underline").and_then(serde_json::Value::as_bool) {
                        let _ = self.run("format.underline", serde_json::json!({"value": u}));
                    }
                    if let Some(sz) = req.params.get("fontSize").or_else(|| req.params.get("size")).and_then(serde_json::Value::as_f64) {
                        let _ = self.run("format.size", serde_json::json!({"size": sz}));
                    }
                    if let Some(col) = req.params.get("color").or_else(|| req.params.get("fontColor")).and_then(serde_json::Value::as_str) {
                        let _ = self.run("format.color", serde_json::json!({"color": col.trim_start_matches('#')}));
                    }
                    Ok(serde_json::json!({"status": "ok"}))
                }
                "execute" => {
                    self.ensure_active_document_for_agent();
                    let raw_cmd = req.params.get("command").and_then(serde_json::Value::as_str).unwrap_or("");
                    let params = req.params.get("params").cloned().unwrap_or(serde_json::json!({}));
                    match raw_cmd {
                        "doc.insertStyledTable" | "insert_styled_table" => self.insert_styled_table_helper(&params),
                        "doc.insertRichLayout" | "doc.generateFancyDocument" => self.execute_rich_layout_spec(&params),
                        "doc.clearAndWrite" => {
                            let _ = self.run("select.all", serde_json::json!({}));
                            let _ = self.run("text.delete", serde_json::json!({}));
                            self.execute_rich_layout_spec(&params)
                        }
                        "doc.replaceSelection" => {
                            let md = params.get("markdown").or_else(|| params.get("text")).and_then(serde_json::Value::as_str).unwrap_or("");
                            self.insert_markdown_into_wordcraft(md, false)
                        }
                        "insert.table" if params.get("headers").is_some() || params.get("data").is_some() => {
                            self.insert_styled_table_helper(&params)
                        }
                        _ => {
                            let cmd = Self::normalize_wordcraft_cmd(raw_cmd);
                            self.run(cmd, params)
                        }
                    }
                }
                "type_text" => {
                    self.ensure_active_document_for_agent();
                    let text = req.params.get("text").and_then(serde_json::Value::as_str).unwrap_or("");
                    if text.contains('\n') || text.contains("# ") || text.contains("**") || text.contains("| ") {
                        self.insert_markdown_into_wordcraft(text, false)
                    } else {
                        self.run("text.insert", serde_json::json!({"text": text}))
                    }
                }
                "select_text" => {
                    self.ensure_active_document_for_agent();
                    let text = req.params.get("text").and_then(serde_json::Value::as_str).unwrap_or("");
                    let _ = self.session.run("edit.find", &serde_json::json!({"text": text}));
                    if let Some((anchor, focus)) = self.session.find.results.first().cloned() {
                        let _ = self.run("select.range", serde_json::json!({"anchor": anchor, "focus": focus}));
                        Ok(serde_json::json!({"selected": true}))
                    } else {
                        Err(format!("Text '{text}' nicht gefunden"))
                    }
                }
                "get_text" => {
                    let text = self.session.doc.plain_text(wordcraft_doc::StoryRef::Body);
                    Ok(serde_json::json!({"text": text}))
                }
                "inspect_document" => {
                    let title = self.title_stem();
                    let words = self.cached_word_count();
                    let dirty = self.session.dirty;
                    let path = self.session.path.as_ref().map(|p| p.to_string_lossy().to_string());
                    Ok(serde_json::json!({
                        "title": title,
                        "word_count": words,
                        "dirty": dirty,
                        "path": path,
                    }))
                }
                "save_document" => {
                    let target = if let Some(p) = req.params.get("path").and_then(serde_json::Value::as_str) {
                        std::path::PathBuf::from(p)
                    } else if let Some(ref path) = self.session.path {
                        path.clone()
                    } else {
                        self.artcraft_drive.set_or_init_project(None)
                    };
                    let r = self.run("file.saveAs", serde_json::json!({"path": target.to_string_lossy().to_string()}));
                    if r.is_ok() {
                        self.artcraft_drive.mark_saved();
                    }
                    r
                }
                other => {
                    self.ensure_active_document_for_agent();
                    let cmd = Self::normalize_wordcraft_cmd(other);
                    self.run(cmd, req.params.clone())
                }
            };
            let _ = req.reply_sender.send(res);
        }

        self.artcraft_agent.process_events();
    }

    /// Automatically saves changes into the active Google Drive project folder if enabled.
    pub fn artcraft_autosave_tick(&mut self) {
        if self.artcraft_drive.auto_save_enabled && self.session.dirty && self.artcraft_drive.should_autosave() {
            let target = if let Some(ref path) = self.session.path {
                path.clone()
            } else {
                self.artcraft_drive.set_or_init_project(None)
            };
            let target_str = target.to_string_lossy().to_string();
            if self.run("file.saveAs", serde_json::json!({"path": target_str})).is_ok() {
                self.artcraft_drive.mark_saved();
                self.status(format!("☁️ Gespeichert in {}", self.artcraft_drive.active_project_folder_name()));
            }
        }
    }
}
