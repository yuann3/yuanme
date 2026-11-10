//! The vercel adapter: the Build Output API's `config.json`.

use yuanme_content::config::Config;

use super::Rule;

/// `.vercel/output/config.json`: yuanme's rules and the Author's, then
/// redirects, then `trailingSlash: true` as Vercel compiles it, then the
/// 404 Page for a path no file serves.
pub(super) fn vercel_config(config: &Config, rules: &[Rule<'_>], not_found: bool) -> String {
    let json = |s: &str| serde_json::to_string(s).expect("a string serializes");
    let mut routes: Vec<String> = Vec::new();
    for rule in rules {
        let headers: Vec<String> = rule
            .headers
            .iter()
            .map(|(n, v, _)| format!("{}: {}", json(n), json(v)))
            .collect();
        routes.push(format!(
            "{{ \"src\": {}, \"headers\": {{ {} }}, \"continue\": true }}",
            json(&regex(rule.path)),
            headers.join(", ")
        ));
    }
    let redirect = |src: &str, to: &str, status: u16| {
        format!(
            "{{ \"src\": {}, \"headers\": {{ \"Location\": {} }}, \"status\": {status} }}",
            json(src),
            json(to)
        )
    };
    for r in &config.host.redirects {
        routes.push(redirect(&regex(&r.from), &r.to, r.status));
    }
    // A file with no extension is only ever in `.well-known`.
    routes.push(format!(
        "{{ \"src\": {} }}",
        json(r"^/\.well-known(?:/.*)?$")
    ));
    routes.push(redirect(r"^/((?:[^/]+/)*[^/\.]+)$", "/$1/", 308));
    routes.push(redirect(r"^/((?:[^/]+/)*[^/]+\.\w+)/$", "/$1", 308));
    routes.push("{ \"handle\": \"filesystem\" }".into());
    if not_found {
        routes.push("{ \"src\": \"/.*\", \"dest\": \"/404.html\", \"status\": 404 }".into());
    }
    format!(
        "{{\n  \"version\": 3,\n  \"routes\": [\n    {}\n  ]\n}}\n",
        routes.join(",\n    ")
    )
}

/// A path with at most one `*`, as an anchored PCRE.
fn regex(path: &str) -> String {
    let mut out = String::from("^");
    for c in path.chars() {
        match c {
            '*' => out.push_str(".*"),
            c if r"\.+?()|[]{}^$".contains(c) => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push('$');
    out
}
