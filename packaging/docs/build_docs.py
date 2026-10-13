#!/usr/bin/env python3
"""Build the user-documentation snapshot of this source tree (docs/DOCS_MAINTENANCE.md).

  build_docs.py --bundle-dir DIR   copy the bundle file set (DOCS_MANIFEST.json "bundle") into DIR,
                                   keeping its layout (docs/ prefix removed), and check that every
                                   relative link inside it resolves inside the bundle
  build_docs.py --html DIR         also render every bundled Markdown file to HTML (needs pandoc)
  build_docs.py --pdf DIR          also print the manual and the Grok guides to PDF (needs pandoc
                                   and a Chromium/Chrome; CHROME=path overrides the search)
  build_docs.py --zip OUT.zip      a self-contained documentation ZIP (Markdown, plus HTML/PDF when
                                   --html/--pdf are given) with START_HERE.md and SHA256SUMS

Run `cargo xtask docs-check` first: this script copies and renders; it does not re-verify content.
Packaging scripts call it with --bundle-dir so every build carries the documentation of its source.
"""
import argparse, hashlib, html, json, os, re, shutil, subprocess, sys, tempfile, zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PDF_DOCS = ["USER_GUIDE.md", "grok/GROK_OPERATOR_GUIDE.md", "grok/GROK_BOT_INSTRUCTIONS.md"]
LINK = re.compile(r"\]\(([^)\s]+)\)")
CSS = """body{font-family:system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem;line-height:1.5;color:#1a1a1a;background:#fff}
table{border-collapse:collapse;margin:1rem 0;font-size:.92em}th,td{border:1px solid #bbb;padding:.3rem .5rem;vertical-align:top;text-align:left}
th{background:#eee}code{font-family:ui-monospace,Consolas,monospace;font-size:.92em;background:#f3f3f3;padding:0 .2em;border-radius:3px}
pre{background:#f3f3f3;padding:.8rem;overflow-x:auto;white-space:pre-wrap}pre code{background:none;padding:0}h1,h2,h3{line-height:1.25}h2{border-bottom:1px solid #ccc;padding-bottom:.2rem;margin-top:2.2rem}
a{color:#0645ad}nav#TOC{background:#f8f8f8;border:1px solid #ddd;padding:.5rem 1rem;margin-bottom:2rem}
@media (prefers-color-scheme:dark){body{background:#151515;color:#e8e8e8}th{background:#2a2a2a}td,th{border-color:#555}code,pre{background:#262626}a{color:#8ab4f8}nav#TOC{background:#1d1d1d;border-color:#444}}
@media print{body{max-width:none;margin:0;font-size:10pt}a{color:#000}nav#TOC{page-break-after:always}h2{page-break-after:avoid}tr,pre{page-break-inside:avoid}}"""


def manifest():
    with open(os.path.join(ROOT, "docs", "DOCS_MANIFEST.json"), encoding="utf-8") as f:
        return json.load(f)


def git_head():
    try:
        return subprocess.run(["git", "-C", ROOT, "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def copy_bundle(m, dest):
    files = []
    for src in m["bundle"]:
        rel = src[len("docs/"):] if src.startswith("docs/") else src
        out = os.path.join(dest, rel)
        os.makedirs(os.path.dirname(out), exist_ok=True)
        shutil.copyfile(os.path.join(ROOT, src), out)
        files.append(rel)
    return files


def check_links(dest, files):
    bad = []
    for rel in files:
        if not rel.endswith(".md"):
            continue
        text = open(os.path.join(dest, rel), encoding="utf-8").read()
        text = re.sub(r"```.*?```", "", text, flags=re.S)
        for target in LINK.findall(text):
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            path = target.split("#", 1)[0]
            resolved = os.path.normpath(os.path.join(os.path.dirname(os.path.join(dest, rel)), path))
            if not resolved.startswith(os.path.abspath(dest)) or not os.path.exists(resolved):
                bad.append(f"{rel}: link `{target}` leaves the bundle or is missing")
    return bad


def render_html(dest, files, out_dir, m):
    title_suffix = f" — SML Studio {m['product_version']} (docs {m['docs_revision']})"
    made = []
    for rel in files:
        if not rel.endswith(".md"):
            continue
        src = os.path.join(dest, rel)
        text = open(src, encoding="utf-8").read()
        # Links between Markdown files point at their HTML renderings.
        text = re.sub(r"\]\(((?!https?:)[^)\s#]+)\.md(#[^)\s]*)?\)", lambda mm: f"]({mm.group(1)}.html{mm.group(2) or ''})", text)
        # Other relative links (JSON, folders) stay with the Markdown files, one level up from html/.
        text = re.sub(r"\]\((?!https?:|mailto:|#)([^)\s]+)\)", lambda mm: f"]({mm.group(1)})" if mm.group(1).split("#")[0].endswith(".html") else f"](../{mm.group(1)})", text)
        tmp = src + ".tmp.md"
        open(tmp, "w", encoding="utf-8").write(text)
        out = os.path.join(out_dir, rel[:-3] + ".html")
        os.makedirs(os.path.dirname(out), exist_ok=True)
        first = next((l[2:].strip() for l in text.splitlines() if l.startswith("# ")), rel)
        css = os.path.join(out_dir, ".style.css")
        open(css, "w", encoding="utf-8").write(CSS)
        args = ["pandoc", tmp, "-f", "gfm", "-t", "html5", "-s", "--embed-resources", "--css", css, "--metadata", f"title={first}{title_suffix}", "--metadata", "lang=en", "-o", out]
        if rel in PDF_DOCS:
            args += ["--toc", "--toc-depth=2"]
        r = subprocess.run(args, capture_output=True, text=True)
        os.remove(tmp)
        os.remove(css)
        if r.returncode != 0 or not os.path.getsize(out):
            raise SystemExit(f"pandoc failed for {rel}: {r.stderr.strip()}")
        body = open(out, encoding="utf-8").read()
        if "<h1" not in body and "<h2" not in body:
            raise SystemExit(f"{rel}: rendered HTML has no headings")
        made.append(out)
    return made


def find_chrome():
    if os.environ.get("CHROME"):
        return os.environ["CHROME"]
    for c in ["chromium", "chromium-browser", "google-chrome", "chrome"]:
        p = shutil.which(c)
        if p:
            return p
    base = "/opt/pw-browsers"
    if os.path.isdir(base):
        for d in sorted(os.listdir(base), reverse=True):
            p = os.path.join(base, d, "chrome-linux", "chrome")
            if d.startswith("chromium-") and os.path.exists(p):
                return p
    return None


def render_pdf(html_dir, out_dir):
    chrome = find_chrome()
    if not chrome:
        raise SystemExit("no Chromium/Chrome found for PDF output (set CHROME=path)")
    made = []
    for rel in PDF_DOCS:
        src = os.path.join(html_dir, rel[:-3] + ".html")
        out = os.path.join(out_dir, os.path.basename(rel)[:-3] + ".pdf")
        os.makedirs(out_dir, exist_ok=True)
        r = subprocess.run([chrome, "--headless", "--no-sandbox", "--disable-gpu", "--no-pdf-header-footer", f"--print-to-pdf={out}", "file://" + os.path.abspath(src)], capture_output=True, text=True, timeout=300)
        data = open(out, "rb").read() if os.path.exists(out) else b""
        pages = len(re.findall(rb"/Type\s*/Page[^s]", data))
        if r.returncode != 0 or not data.startswith(b"%PDF") or pages < 1:
            raise SystemExit(f"PDF failed for {rel}: rc={r.returncode} pages={pages} {r.stderr.strip()[-300:]}")
        made.append((out, pages))
    return made


START_HERE = """# SML Studio documentation — start here

Version {version}, documentation revision {rev}, built from source `{head}`.
The documentation describes source `{verified}` (see DOCS_MANIFEST.json → verification).

## Loading this into your Grok bot
1. Paste the text block from `grok/GROK_BOT_INSTRUCTIONS.md` into the bot's instructions
   (system prompt / custom instructions field of your Grok setup).
2. Give the bot these files as reference material (knowledge files / attachments), at least:
   `USER_GUIDE.md`, `grok/GROK_OPERATOR_GUIDE.md`, `COMMAND_REFERENCE.md`, `reference/commands.json`,
   `reference/CAPABILITIES.md`, `reference/SHORTCUTS.md`, `DOCS_MANIFEST.json`.
   If the bot only accepts PDFs, use the files in `pdf/`; for reading in a browser, `html/`.
3. If the bot can run programs on this computer, it may use `soundcraft-cli.exe` or the local
   control channel; you decide whether to enable that (`grok/GROK_OPERATOR_GUIDE.md` §2).
4. At the start of each task the bot checks your installed build against
   `DOCS_MANIFEST.json` → `known_builds`. When you install a new build, give the bot the
   documentation ZIP of that build.

## For you
- `README.md` — index. `USER_GUIDE.md` — the manual. `html/USER_GUIDE.html` — the same in a browser.
- SHA256SUMS lists a checksum for every file in this ZIP.
"""


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bundle-dir")
    ap.add_argument("--html", action="store_true")
    ap.add_argument("--pdf", action="store_true")
    ap.add_argument("--zip")
    a = ap.parse_args()
    if not (a.bundle_dir or a.zip):
        ap.error("give --bundle-dir DIR and/or --zip OUT.zip")
    m = manifest()
    work = tempfile.mkdtemp(prefix="sml-docs-")
    try:
        files = copy_bundle(m, work)
        bad = check_links(work, files)
        if bad:
            raise SystemExit("bundle links:\n  " + "\n  ".join(bad))
        report = [f"bundle: {len(files)} files, links ok"]
        if a.html or a.pdf:
            html_dir = os.path.join(work, "html")
            made = render_html(work, files, html_dir, m)
            report.append(f"html: {len(made)} pages")
            if a.pdf:
                pdfs = render_pdf(html_dir, os.path.join(work, "pdf"))
                report.append("pdf: " + ", ".join(f"{os.path.basename(p)} ({n} pages)" for p, n in pdfs))
        head = git_head()
        open(os.path.join(work, "START_HERE.md"), "w", encoding="utf-8").write(
            START_HERE.format(version=m["product_version"], rev=m["docs_revision"], head=head, verified=m["verification"]["source_sha"])
        )
        sums = []
        for dp, _, fs in os.walk(work):
            for f in sorted(fs):
                p = os.path.join(dp, f)
                sums.append(f"{hashlib.sha256(open(p, 'rb').read()).hexdigest()}  {os.path.relpath(p, work).replace(os.sep, '/')}")
        open(os.path.join(work, "SHA256SUMS"), "w", encoding="utf-8").write("\n".join(sorted(sums, key=lambda l: l[66:])) + "\n")
        if a.bundle_dir:
            if os.path.exists(a.bundle_dir):
                shutil.rmtree(a.bundle_dir)
            shutil.copytree(work, a.bundle_dir)
            report.append(f"copied to {a.bundle_dir}")
        if a.zip:
            name = f"sml-studio-docs-{m['product_version']}-{m['docs_revision']}"
            with zipfile.ZipFile(a.zip, "w", zipfile.ZIP_DEFLATED) as z:
                for dp, _, fs in os.walk(work):
                    for f in sorted(fs):
                        p = os.path.join(dp, f)
                        info = zipfile.ZipInfo(f"{name}/{os.path.relpath(p, work).replace(os.sep, '/')}", date_time=(2026, 1, 1, 0, 0, 0))
                        info.compress_type = zipfile.ZIP_DEFLATED
                        info.external_attr = 0o644 << 16
                        z.writestr(info, open(p, "rb").read())
            with zipfile.ZipFile(a.zip) as z:
                broken = z.testzip()
                if broken:
                    raise SystemExit(f"zip test failed at {broken}")
                count = len(z.namelist())
            digest = hashlib.sha256(open(a.zip, "rb").read()).hexdigest()
            report.append(f"zip: {a.zip} ({count} files, sha256 {digest})")
        print("\n".join(report))
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
