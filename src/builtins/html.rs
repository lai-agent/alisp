use crate::Evaluator;
use crate::expr::{Expr, expr_to_string};
use scraper::{Html, Selector};

/// Parse HTML and return a scraper::Html document
fn parse_html(html: &str) -> Html {
    Html::parse_document(html)
}

/// Extract text content from an element and its descendants
fn element_text(element: &scraper::ElementRef) -> String {
    let mut texts = Vec::new();
    for node in element.descendants() {
        if let scraper::Node::Text(text) = node.value() {
            let t = text.trim();
            if !t.is_empty() {
                texts.push(t.to_string());
            }
        }
    }
    texts.join(" ")
}

impl Evaluator {
    /// (html-select html css-selector)
    /// Select elements using CSS selector. Returns list of HTML strings.
    pub(crate) fn builtin_html_select(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.len() < 2 {
            return Err("(html-select html css-selector) requires 2 arguments".into());
        }

        let html_str = expr_to_string(&args[0]);
        let css = expr_to_string(&args[1]);

        let document = parse_html(&html_str);
        let selector = Selector::parse(&css)
            .map_err(|e| format!("html-select: invalid selector '{}': {}", css, e))?;

        let results: Vec<Expr> = document
            .select(&selector)
            .map(|el| Expr::Str(el.html()))
            .collect();

        Ok(Expr::List(results))
    }

    /// (html-text html css-selector)
    /// Extract text content from elements matching CSS selector.
    pub(crate) fn builtin_html_text(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.len() < 2 {
            return Err("(html-text html css-selector) requires 2 arguments".into());
        }

        let html_str = expr_to_string(&args[0]);
        let css = expr_to_string(&args[1]);

        let document = parse_html(&html_str);
        let selector = Selector::parse(&css)
            .map_err(|e| format!("html-text: invalid selector '{}': {}", css, e))?;

        let results: Vec<Expr> = document
            .select(&selector)
            .map(|el| Expr::Str(element_text(&el)))
            .collect();

        Ok(Expr::List(results))
    }

    /// (html-attr html css-selector attr-name)
    /// Extract attribute values from elements matching CSS selector.
    pub(crate) fn builtin_html_attr(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.len() < 3 {
            return Err("(html-attr html css-selector attr-name) requires 3 arguments".into());
        }

        let html_str = expr_to_string(&args[0]);
        let css = expr_to_string(&args[1]);
        let attr_name = expr_to_string(&args[2]);

        let document = parse_html(&html_str);
        let selector = Selector::parse(&css)
            .map_err(|e| format!("html-attr: invalid selector '{}': {}", css, e))?;

        let results: Vec<Expr> = document
            .select(&selector)
            .filter_map(|el| {
                el.value()
                    .attr(&attr_name)
                    .map(|v| Expr::Str(v.to_string()))
            })
            .collect();

        Ok(Expr::List(results))
    }

    /// (html-links html)
    /// Extract all links (href) from an HTML document.
    /// Returns list of ((text url) ...) pairs.
    pub(crate) fn builtin_html_links(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.is_empty() {
            return Err("(html-links html) requires 1 argument".into());
        }

        let html_str = expr_to_string(&args[0]);
        let document = parse_html(&html_str);
        let selector = Selector::parse("a[href]")
            .map_err(|e| format!("html-links: selector error: {}", e))?;

        let results: Vec<Expr> = document
            .select(&selector)
            .filter_map(|el| {
                let href = el.value().attr("href")?;
                let text = element_text(&el);
                Some(Expr::List(vec![
                    Expr::Str(text),
                    Expr::Str(href.to_string()),
                ]))
            })
            .collect();

        Ok(Expr::List(results))
    }

    /// (html-title html)
    /// Extract the page title from an HTML document.
    pub(crate) fn builtin_html_title(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.is_empty() {
            return Err("(html-title html) requires 1 argument".into());
        }

        let html_str = expr_to_string(&args[0]);
        let document = parse_html(&html_str);

        let selector = Selector::parse("title")
            .map_err(|e| format!("html-title: selector error: {}", e))?;

        match document.select(&selector).next() {
            Some(el) => Ok(Expr::Str(element_text(&el))),
            None => Ok(Expr::Nil),
        }
    }

    /// (html-meta html name)
    /// Extract meta tag content by name or property.
    /// Returns the content attribute value, or nil if not found.
    pub(crate) fn builtin_html_meta(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.len() < 2 {
            return Err("(html-meta html name) requires 2 arguments".into());
        }

        let html_str = expr_to_string(&args[0]);
        let name = expr_to_string(&args[1]);
        let document = parse_html(&html_str);

        // Try name attribute first, then property (for og: tags)
        let selector_str = format!("meta[name=\"{}\"], meta[property=\"{}\"]", name, name);
        let selector = Selector::parse(&selector_str)
            .map_err(|e| format!("html-meta: selector error: {}", e))?;

        match document.select(&selector).next() {
            Some(el) => match el.value().attr("content") {
                Some(content) => Ok(Expr::Str(content.to_string())),
                None => Ok(Expr::Nil),
            },
            None => Ok(Expr::Nil),
        }
    }

    /// (html-meta-all html)
    /// Extract all meta tags as ((name content) ...) pairs.
    pub(crate) fn builtin_html_meta_all(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.is_empty() {
            return Err("(html-meta-all html) requires 1 argument".into());
        }

        let html_str = expr_to_string(&args[0]);
        let document = parse_html(&html_str);

        let selector = Selector::parse("meta")
            .map_err(|e| format!("html-meta-all: selector error: {}", e))?;

        let results: Vec<Expr> = document
            .select(&selector)
            .filter_map(|el| {
                let name = el
                    .value()
                    .attr("name")
                    .or_else(|| el.value().attr("property"))?;
                let content = el.value().attr("content")?;
                Some(Expr::List(vec![
                    Expr::Str(name.to_string()),
                    Expr::Str(content.to_string()),
                ]))
            })
            .collect();

        Ok(Expr::List(results))
    }

    /// (html-tables html)
    /// Extract all tables from HTML. Returns list of tables,
    /// each table is a list of rows, each row is a list of cell values.
    pub(crate) fn builtin_html_tables(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.is_empty() {
            return Err("(html-tables html) requires 1 argument".into());
        }

        let html_str = expr_to_string(&args[0]);
        let document = parse_html(&html_str);

        let table_sel = Selector::parse("table").unwrap();
        let row_sel = Selector::parse("tr").unwrap();
        let cell_sel = Selector::parse("td, th").unwrap();

        let tables: Vec<Expr> = document
            .select(&table_sel)
            .map(|table| {
                let rows: Vec<Expr> = table
                    .select(&row_sel)
                    .map(|row| {
                        let cells: Vec<Expr> = row
                            .select(&cell_sel)
                            .map(|cell| Expr::Str(element_text(&cell)))
                            .collect();
                        Expr::List(cells)
                    })
                    .collect();
                Expr::List(rows)
            })
            .collect();

        Ok(Expr::List(tables))
    }

    /// (html-images html)
    /// Extract all images as ((alt src) ...) pairs.
    pub(crate) fn builtin_html_images(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.is_empty() {
            return Err("(html-images html) requires 1 argument".into());
        }

        let html_str = expr_to_string(&args[0]);
        let document = parse_html(&html_str);

        let selector = Selector::parse("img")
            .map_err(|e| format!("html-images: selector error: {}", e))?;

        let results: Vec<Expr> = document
            .select(&selector)
            .map(|el| {
                let alt = el.value().attr("alt").unwrap_or("");
                let src = el.value().attr("src").unwrap_or("");
                Expr::List(vec![
                    Expr::Str(alt.to_string()),
                    Expr::Str(src.to_string()),
                ])
            })
            .collect();

        Ok(Expr::List(results))
    }

    /// (html-forms html)
    /// Extract forms with their inputs.
    /// Returns list of ((action method (input-name type value) ...) ...)
    pub(crate) fn builtin_html_forms(&mut self, args: &[Expr]) -> Result<Expr, String> {
        if args.is_empty() {
            return Err("(html-forms html) requires 1 argument".into());
        }

        let html_str = expr_to_string(&args[0]);
        let document = parse_html(&html_str);

        let form_sel = Selector::parse("form").unwrap();
        let input_sel = Selector::parse("input, select, textarea").unwrap();

        let forms: Vec<Expr> = document
            .select(&form_sel)
            .map(|form| {
                let action = form.value().attr("action").unwrap_or("").to_string();
                let method = form
                    .value()
                    .attr("method")
                    .unwrap_or("GET")
                    .to_uppercase();

                let inputs: Vec<Expr> = form
                    .select(&input_sel)
                    .map(|input| {
                        let name = input.value().attr("name").unwrap_or("").to_string();
                        let input_type = input.value().attr("type").unwrap_or("text").to_string();
                        let value = input.value().attr("value").unwrap_or("").to_string();
                        Expr::List(vec![
                            Expr::Str(name),
                            Expr::Str(input_type),
                            Expr::Str(value),
                        ])
                    })
                    .collect();

                Expr::List(vec![
                    Expr::Str(action),
                    Expr::Str(method),
                    Expr::List(inputs),
                ])
            })
            .collect();

        Ok(Expr::List(forms))
    }
}
