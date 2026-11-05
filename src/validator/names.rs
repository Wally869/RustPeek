use super::*;

impl<'ast> Visit<'ast> for VisPathCheck<'_> {
    fn visit_item_mod(&mut self, m: &'ast syn::ItemMod) {
        self.visit_visibility(&m.vis);
        if let Some((_, inner)) = &m.content {
            let child = self.module_path.child(&m.ident.to_string());
            let outer = std::mem::replace(&mut self.module_path, child);
            for item in inner {
                self.visit_item(item);
            }
            self.module_path = outer;
        }
    }

    fn visit_vis_restricted(&mut self, r: &'ast syn::VisRestricted) {
        if resolve_vis_path(&r.path, &self.module_path).is_some() {
            return;
        }
        let path_text = r
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");
        let span = r.path.segments[0].ident.span();
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            file: self.file_path.to_path_buf(),
            line: span.start().line,
            column: span.start().column + 1,
            message: format!(
                "failed to resolve `{path_text}`: there are too many leading `super` keywords"
            ),
            error_code: Some("E0433".to_string()),
            hint: None,
            fix: None,
        });
    }
}

pub(super) fn generic_param_names(generics: &syn::Generics) -> Vec<String> {
    generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Type(t) => Some(t.ident.to_string()),
            syn::GenericParam::Const(c) => Some(c.ident.to_string()),
            syn::GenericParam::Lifetime(_) => None,
        })
        .collect()
}

pub(super) fn pattern_names(pat: &syn::Pat) -> Vec<String> {
    struct Collector(Vec<String>);
    impl<'ast> Visit<'ast> for Collector {
        fn visit_pat_ident(&mut self, node: &'ast syn::PatIdent) {
            self.0.push(node.ident.to_string());
            syn::visit::visit_pat_ident(self, node);
        }
    }
    let mut collector = Collector(Vec::new());
    collector.visit_pat(pat);
    collector.0
}

pub(super) fn signature_names(sig: &syn::Signature) -> Vec<String> {
    let mut names = generic_param_names(&sig.generics);
    for arg in &sig.inputs {
        match arg {
            syn::FnArg::Receiver(_) => names.push("self".to_string()),
            syn::FnArg::Typed(typed) => names.extend(pattern_names(&typed.pat)),
        }
    }
    names
}

/// Levenshtein edit distance between two strings.
pub(super) fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();

    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j] + cost)
                .min(prev[j + 1] + 1)
                .min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_len]
}

impl<'a> ValidationVisitor<'a> {
    /// Get source lines, loading lazily.
    pub(super) fn source_lines(&mut self) -> &[String] {
        if self.source_lines.is_none() {
            let content = std::fs::read_to_string(self.file_path).unwrap_or_default();
            self.source_lines = Some(content.lines().map(String::from).collect());
        }
        self.source_lines.as_deref().unwrap()
    }

    /// A `mod x;` with a sibling `mod x;` under a different `#[cfg]` whose file exists. rustc
    /// loads only the active one; the tool cannot evaluate cfg, so the missing file is
    /// accepted.
    pub(super) fn has_cfg_twin(&self, items: &[syn::Item], m: &syn::ItemMod) -> bool {
        let cfgs = |module: &syn::ItemMod| crate::types::cfg_conditions(&module.attrs);
        let own = cfgs(m);
        items.iter().any(|item| match item {
            syn::Item::Mod(other) => {
                other.ident == m.ident
                    && other.content.is_none()
                    && cfgs(other) != own
                    && discovery::resolve_mod_file(self.file_path, self.owned_dir, false, other)
                        .is_some()
            }
            _ => false,
        })
    }

}

impl<'a, 'b> RefVisitor<'a, 'b> {
    pub(super) fn with_generics(&mut self, generics: &syn::Generics, visit: impl FnOnce(&mut Self)) {
        self.with_names(generic_param_names(generics), visit);
    }

    pub(super) fn with_names(&mut self, names: Vec<String>, visit: impl FnOnce(&mut Self)) {
        self.validator.local_scopes.push(LocalScope { names, ..LocalScope::default() });
        visit(self);
        self.validator.local_scopes.pop();
    }

}

impl LocalScope {
    /// Whether the scope binds `name` without a path to resolve.
    pub(super) fn binds(&self, name: &str) -> bool {
        self.has_macro_items || self.names.iter().any(|n| n == name)
    }

}
