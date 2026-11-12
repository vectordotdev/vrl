import contextlib
import io
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

import check_deny


def diagnostic(advisory="RUSTSEC-2026-0001", version="1.0.0", **overrides):
    fields = {
        "severity": "error", "code": "vulnerability", "message": "vulnerable crate",
        "advisory": {"id": advisory} if advisory else None,
        "graphs": [{"Krate": {"name": "example", "version": version}}],
    }
    fields.update(overrides)
    return {"type": "diagnostic", "fields": fields}


def output(*diagnostics, status=None, summary=True):
    errors = sum(item["fields"]["severity"] == "error" for item in diagnostics)
    entries = list(diagnostics)
    if summary:
        entries.append({"type": "summary", "fields": {
            "advisories": {"errors": errors, "warnings": 0},
        }})
    return subprocess.CompletedProcess(
        args=[], returncode=int(bool(errors)) if status is None else status,
        stdout="", stderr="\n".join(json.dumps(entry) for entry in entries),
    )


class DenyComparisonTests(unittest.TestCase):
    def compare(self, head, base=None):
        results = [head] if base is None else [head, base]
        with patch("check_deny.subprocess.run", side_effect=results) as run:
            with contextlib.redirect_stdout(io.StringIO()) as log:
                result = check_deny.check(Path("base"))
        return result, log.getvalue(), run

    def test_clean_head_does_not_need_base(self):
        result, _, run = self.compare(output())
        self.assertEqual(result, 0)
        self.assertEqual(run.call_count, 1)

    def test_existing_advisory_passes_across_versions(self):
        result, log, run = self.compare(
            output(diagnostic(version="1.0.1")), output(diagnostic()),
        )
        self.assertEqual(result, 0)
        self.assertIn("Existing error (ignored)", log)
        self.assertEqual(run.call_args_list[0].kwargs["cwd"], Path.cwd())
        self.assertEqual(run.call_args_list[1].kwargs["cwd"], Path("base"))
        self.assertEqual(run.call_args_list[0].args, run.call_args_list[1].args)

    def test_new_advisory_fails_alongside_existing_one(self):
        result, log, _ = self.compare(
            output(diagnostic(), diagnostic(advisory="RUSTSEC-2026-0002")),
            output(diagnostic()),
        )
        self.assertEqual(result, 1)
        self.assertIn("New error:", log)
        self.assertIn("RUSTSEC-2026-0002", log)

    def test_removing_an_error_passes(self):
        result, _, _ = self.compare(
            output(diagnostic()),
            output(diagnostic(), diagnostic(advisory="RUSTSEC-2026-0002")),
        )
        self.assertEqual(result, 0)

    def test_existing_license_error_passes(self):
        error = diagnostic(None, code="rejected", message="license is not allowed")
        self.assertEqual(self.compare(output(error), output(error))[0], 0)

    def test_new_license_error_for_same_crate_fails(self):
        self.assertEqual(self.compare(
            output(diagnostic(None, code="rejected", message="new rejected license")),
            output(diagnostic(None, code="rejected", message="old rejected license")),
        )[0], 1)

    def test_different_rejected_license_is_a_new_error(self):
        def license_error(license):
            return diagnostic(None, code="rejected", message="failed to satisfy license requirements",
                              labels=[{"message": "rejected", "span": license}])
        self.assertEqual(self.compare(
            output(license_error("MIT")), output(license_error("BSD-3-Clause")),
        )[0], 1)

    def test_positive_error_status_depends_on_check_type(self):
        self.assertEqual(self.compare(
            output(diagnostic(None, code="rejected"), status=4),
            output(diagnostic(None, code="rejected"), status=4),
        )[0], 0)

    def test_warnings_do_not_fail(self):
        self.assertEqual(self.compare(output(diagnostic(severity="warning")))[0], 0)

    def test_incomplete_scans_fail_for_head_and_base(self):
        for results in [
            [output(status=1, summary=False)],
            [output(diagnostic()), output(status=1, summary=False)],
        ]:
            with self.subTest(results=results):
                with patch("check_deny.subprocess.run", side_effect=results):
                    with self.assertRaisesRegex(RuntimeError, "did not complete reliably"):
                        check_deny.check(Path("base"))

    def test_status_must_agree_with_diagnostics(self):
        for result in [output(status=1), output(diagnostic(), status=0),
                       output(diagnostic(), status=-9)]:
            with self.subTest(status=result.returncode):
                with patch("check_deny.subprocess.run", return_value=result):
                    with self.assertRaises(RuntimeError):
                        check_deny.deny_errors(Path("head"))

    def test_malformed_output_fails(self):
        malformed = ["not json", json.dumps({"type": "diagnostic", "fields": {
            "severity": "error", "code": "rejected",
        }})]
        for stderr in malformed:
            with self.subTest(stderr=stderr):
                result = subprocess.CompletedProcess([], 1, "", stderr)
                with patch("check_deny.subprocess.run", return_value=result):
                    with self.assertRaises(RuntimeError):
                        check_deny.deny_errors(Path("head"))

    def test_summary_errors_cannot_be_silently_ignored(self):
        result = output(diagnostic())
        result.stderr = result.stderr.splitlines()[-1]
        with patch("check_deny.subprocess.run", return_value=result):
            with self.assertRaises(RuntimeError):
                check_deny.deny_errors(Path("head"))

    def test_self_comparison_is_rejected(self):
        with patch("sys.argv", ["check_deny.py", "--base-dir", "."]):
            with contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as error:
                    check_deny.main()
        self.assertEqual(error.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
