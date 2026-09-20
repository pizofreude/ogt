//! Pure text/shape logic for PATH shims: the shim script template, and the
//! marker ogt uses to recognize its own generated files. No filesystem
//! I/O here — see `init::shim_fs` for that, matching `settings_edit`'s
//! split from `fs_ops`.
//!
//! Every shim references ogt by the absolute path of the currently
//! running executable, so it keeps working regardless of `PATH` order —
//! never a bare `ogt`, which the shim's own directory could shadow. Each
//! shim also exports `OGT_SHIM_DIR` to its own directory before invoking
//! ogt; see `crate::cli::path_shim` for why that is required to avoid
//! infinite recursion.

/// Fixed marker every ogt-generated shim carries, as its own comment
/// line — so uninstall can tell ogt's own files apart from anything
/// else a user put in the shim directory. Checked as an exact-line match,
/// never a substring guess.
const MARKER: &str = "ogt shim v1";

/// Render one shim script for `program`, invoking `ogt_exe` (the
/// absolute path of the running ogt binary, from
/// `std::env::current_exe()`).
///
/// `OGT_SHIM_DIR` must be expressed in the same form the platform's
/// `PATH` uses, because `cli::path_shim` removes it from `PATH` by exact
/// string comparison against `std::env::split_paths`. On Unix `cd && pwd`
/// already yields that form. Under MSYS/Git Bash it does not: a native
/// Windows `ogt.exe` receives `PATH` with Windows entries (`C:\Users\...`),
/// while `cd && pwd` yields a POSIX path (`/c/Users/...`). The comparison
/// then never matches, the recursion guard silently fails, ogt resolves
/// `program` back to its own shim, and the spawn dies with
/// `%1 is not a valid Win32 application. (os error 193)`. Normalizing
/// through `cygpath -w` when it is on `PATH` keeps both sides in
/// agreement. On Unix `cygpath` is absent and the branch is skipped, so
/// this stays a no-op there.
pub(crate) fn render_shim(ogt_exe: &str, program: &str) -> String {
    use crate::cli::OGT_SHIM_DIR as VAR;
    format!(
        "#!/bin/sh\n\
         # {MARKER}\n\
         {VAR}=\"$(cd \"$(dirname \"$0\")\" && pwd)\"\n\
         if command -v cygpath >/dev/null 2>&1; then\n\
         {VAR}=\"$(cygpath -w \"${{{VAR}}}\")\"\n\
         fi\n\
         export {VAR}\n\
         exec \"{ogt_exe}\" {program} \"$@\"\n"
    )
}

/// `true` if `contents` is a shim ogt generated — an exact-line match on
/// the marker comment, never a loose substring check that could misfire
/// on a user's own comment.
pub(crate) fn is_ogt_shim(contents: &str) -> bool {
    let marker_line = format!("# {MARKER}");
    contents.lines().any(|line| line.trim() == marker_line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_shim_carries_marker_and_absolute_ogt_path() {
        let script = render_shim("/abs/path/to/ogt", "grep");
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(is_ogt_shim(&script));
        assert!(script.contains("exec \"/abs/path/to/ogt\" grep \"$@\""));
        assert!(script.contains("OGT_SHIM_DIR"));
    }

    #[test]
    fn render_shim_is_specific_to_its_program() {
        let grep = render_shim("/abs/ogt", "grep");
        let cat = render_shim("/abs/ogt", "cat");
        assert_ne!(grep, cat);
        assert!(grep.contains(" grep \"$@\""));
        assert!(cat.contains(" cat \"$@\""));
    }

    /// The recursion guard in `cli::path_shim` strips `OGT_SHIM_DIR` from
    /// `PATH` by exact string comparison against `env::split_paths`. On
    /// Windows that form is native (`C:\...`, `;`-separated), so a POSIX-only
    /// shim dir can never match, and the guard fails silently by resolving the
    /// program back to the shim itself (os error 193). The rendered shim must
    /// therefore normalize through `cygpath` when it is available.
    #[test]
    fn render_shim_normalizes_the_shim_dir_to_the_platform_form() {
        let script = render_shim("/abs/ogt", "grep");
        assert!(
            script.contains("command -v cygpath"),
            "shim must probe for cygpath:\n{script}"
        );
        assert!(
            script.contains("cygpath -w"),
            "shim must normalize OGT_SHIM_DIR to the Windows form:\n{script}"
        );
        assert!(
            script.contains("OGT_SHIM_DIR=\"$(cygpath -w \"${OGT_SHIM_DIR}\")\""),
            "normalization must rewrite OGT_SHIM_DIR in place:\n{script}"
        );
    }

    #[test]
    fn is_ogt_shim_rejects_unmarked_content() {
        assert!(!is_ogt_shim("#!/bin/sh\necho hi\n"));
    }

    #[test]
    fn is_ogt_shim_rejects_a_loose_substring_match() {
        assert!(!is_ogt_shim(
            "#!/bin/sh\necho 'ogt shim v1 mentioned here'\n"
        ));
    }
}
