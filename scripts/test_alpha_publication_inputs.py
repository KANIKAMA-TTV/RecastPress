import copy
import unittest

from alpha_publication_path import public_environment, validate_inputs


class AlphaPublicationInputsTests(unittest.TestCase):
    def test_editing_credentials_are_removed_case_insensitively_without_mutating_parent(self):
        environment = {"PATH": "runtime", "SystemRoot": "os", "RECASTPRESS_DB_PASSWORD": "SECRET",
            "mysql_pwd": "SECRET", "MARIADB_HOST": "private", "mariadb_password": "SECRET",
            "Database_Url": "SECRET", "RECASTPRESS_ALPHA_POST_PATH": "disposable"}
        before = dict(environment)
        self.assertEqual(public_environment(environment), {"PATH": "runtime", "SystemRoot": "os"})
        self.assertEqual(environment, before)

    def test_only_explicit_selected_text_projection_is_admitted(self):
        value = {phase: {"format_version": 1, "posts": [{"target": "/alpha-post", "title": phase,
            "body_text": "literal <text> & Unicode 日"}]} for phase in ("a", "b")}
        before = copy.deepcopy(value)
        validate_inputs(value)
        self.assertEqual(value, before)
        variants = []
        private = copy.deepcopy(value); private["a"]["posts"][0]["post_password"] = "SECRET"; variants.append(private)
        boolean = copy.deepcopy(value); boolean["a"]["format_version"] = True; variants.append(boolean)
        identical = copy.deepcopy(value); identical["b"] = identical["a"]; variants.append(identical)
        extra = copy.deepcopy(value); extra["private_metadata"] = {}; variants.append(extra)
        wrong_target = copy.deepcopy(value); wrong_target["a"]["posts"][0]["target"] = "/other"; variants.append(wrong_target)
        null = copy.deepcopy(value); null["a"]["posts"][0]["body_text"] = None; variants.append(null)
        for item in variants:
            with self.subTest(value=item), self.assertRaises(ValueError):
                validate_inputs(item)


if __name__ == "__main__":
    unittest.main()
