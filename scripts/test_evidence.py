import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import compatibility_registry as registry_tool
import wordpress_reference as reference
import wordpress_entity_names as entity_names
import wordpress_xml_entity_mapping as xml_entities
import wp_bcrypt_profile as wp_bcrypt
import wp_rehash_profile as wp_rehash
import remove_accents_profile as accent_profile
import user_sanitization_profile as user_profile
import slug_byte_profile as slug_profile
import sanitize_title_profile as default_title_profile
import truncate_slug_profile as truncate_profile


class EvidenceTests(unittest.TestCase):
    def test_truncate_slug_profile_requires_conditional_utf8_zero_length_and_final_cap(self):
        expected=truncate_profile.PROFILE
        registry_tool.validate_truncate_slug_profile(dict(expected))
        for invalid in (None,{}, {**expected,'raw_utf8_required':True}):
            with self.assertRaises(ValueError):registry_tool.validate_truncate_slug_profile(invalid)
        for key,original in expected.items():
            for invalid in (None,0.0,'changed',not original if type(original)is bool else True):
                with self.subTest(key=key,invalid=invalid),self.assertRaises(ValueError):registry_tool.validate_truncate_slug_profile({**expected,key:invalid})

    def test_truncate_slug_requires_actual_three_branch_counts_and_old71_exact_results(self):
        branches=dict.fromkeys(truncate_profile.BRANCHES,1)
        args=[3,branches,dict(branches),truncate_profile.OLD_INPUT_SHA256,truncate_profile.OLD_OUTPUT_SHA256,3]
        registry_tool.validate_truncate_slug_coverage(*args)
        registry_tool.validate_truncate_slug_calls(0,dict.fromkeys(branches,0),0)
        for position,value in [(0,True),(0,2),(1,{}),(1,{**branches,'raw_substr':False}),
            (2,{**branches,'decoded_uri':0}),(2,{**branches,'raw_substr':1.0}),(3,None),(4,'stale'),(5,True)]:
            changed=list(args);changed[position]=value
            with self.subTest(position=position,value=value),self.assertRaises(ValueError):registry_tool.validate_truncate_slug_coverage(*changed)
    def test_default_title_profile_requires_exact_callback_no_intl_raw_fallback_and_host_cap(self):
        expected=default_title_profile.PROFILE
        registry_tool.validate_default_title_profile(dict(expected))
        for invalid in (None,{}, {**expected,'arbitrary_filters':True}):
            with self.assertRaises(ValueError):registry_tool.validate_default_title_profile(invalid)
        for key,original in expected.items():
            for invalid in (None,0.0,'changed',not original if type(original)is bool else True):
                with self.subTest(key=key,invalid=invalid),self.assertRaises(ValueError):registry_tool.validate_default_title_profile({**expected,key:invalid})

    def test_default_title_counts_require_contexts_conditional_locale_restoration_and_old70_results(self):
        args=[3,3,{'zero':2,'one':1},{'zero':2,'one':1},3,{'save':1,'display':1,'query':1},default_title_profile.OLD_INPUT_SHA256,default_title_profile.OLD_OUTPUT_SHA256,3]
        registry_tool.validate_default_title_coverage(*args)
        registry_tool.validate_default_title_calls(0,0,{'zero':0,'one':0},0,0)
        for index,value in [(0,True),(0,2),(1,False),(1,2),(2,{}),(2,{'zero':True,'one':2}),
            (3,{'zero':1,'one':2}),(4,False),(4,2),(5,{'save':3}),(5,{'save':True,'display':1,'query':1}),
            (6,None),(7,'stale'),(8,True)]:
            changed=list(args);changed[index]=value
            with self.subTest(index=index,value=value),self.assertRaises(ValueError):registry_tool.validate_default_title_coverage(*changed)
    def test_uri_title_profiles_reject_mb_functions_context_and_type_drift(self):
        for expected,title in ((slug_profile.URI_PROFILE,False),(slug_profile.TITLE_PROFILE,True)):
            registry_tool.validate_slug_profile(dict(expected),title)
            for invalid in (None,{}, {**expected,'unicode_casefold':True}):
                with self.assertRaises(ValueError):registry_tool.validate_slug_profile(invalid,title)
            for key,value in expected.items():
                for bad in (None,0.0,'different',True if type(value)is not bool else not value):
                    with self.subTest(key=key,bad=bad),self.assertRaises(ValueError):
                        registry_tool.validate_slug_profile({**expected,key:bad},title)

    def test_uri_title_coverage_requires_actual_calls_all_cells_and_old68_exact_results(self):
        registry_tool.validate_slug_coverage(4,{'zero:false':1,'zero:true':1,'positive:false':1,'positive:true':1},4)
        registry_tool.validate_slug_coverage(2,{'display':1,'save':1},2,True)
        for calls,cells,total,title in [(True,{'display':1,'save':1},2,True),(1,{'display':1,'save':1},2,True),
            (2,{'display':2},2,True),(2,{'display':True,'save':1},2,True),
            (4,{'zero:false':2,'positive:true':2},4,False),(4,{'zero:false':1,'zero:true':1,'positive:false':1,'positive:true':0},4,False)]:
            with self.assertRaises(ValueError):registry_tool.validate_slug_coverage(calls,cells,total,title)
        registry_tool.validate_previous68(slug_profile.OLD_INPUT_SHA256,slug_profile.OLD_OUTPUT_SHA256)
        for first,second in ((None,slug_profile.OLD_OUTPUT_SHA256),(slug_profile.OLD_INPUT_SHA256,'changed')):
            with self.assertRaises(ValueError):registry_tool.validate_previous68(first,second)
    def test_no_lt_strip_and_user_profiles_reject_coercion_ambient_locale_and_normalizer_drift(self):
        for expected,validate in ((user_profile.STRIP_PROFILE,registry_tool.validate_strip_all_tags_profile),
                                  (user_profile.USER_PROFILE,registry_tool.validate_sanitize_user_profile)):
            validate(dict(expected))
            for bad in (None,{}, {**expected,'general_html_parser':True}):
                with self.assertRaises(ValueError):validate(bad)
            for key,original in expected.items():
                for value in (None,0.0,'different',not original if type(original) is bool else True):
                    with self.subTest(key=key,value=value),self.assertRaises(ValueError):validate({**expected,key:value})

    def test_user_metadata_requires_filter_totals_both_locale_paths_exact_zero_db_and_prior_corpus(self):
        args=[2,2,{'zero':1,'one':1},{'zero':1,'one':1},0,{'false':1,'true':1},user_profile.ACCENT_INPUT_SHA256,2]
        registry_tool.validate_sanitize_user_coverage(*args)
        registry_tool.validate_sanitize_user_calls(0,0,{'zero':0,'one':0},0,0)
        for position,value in [(0,True),(0,1),(1,False),(1,1),(2,{}),(2,{'zero':True,'one':1}),
            (3,{'zero':0,'one':2}),(3,{'zero':1.0,'one':1}),(4,False),(4,1),(5,{'false':2}),(5,{'false':True,'true':1}),
            (6,None),(6,'wrong'),(7,True),(7,0)]:
            changed=list(args);changed[position]=value
            with self.assertRaises(ValueError):registry_tool.validate_sanitize_user_coverage(*changed)

    def test_remove_accents_profile_requires_pinned_tables_explicit_locale_and_both_normalizers_absent(self):
        registry_tool.validate_remove_accents_profile(dict(accent_profile.PROFILE))
        for bad in (None,{}, {**accent_profile.PROFILE,'ambient_locale':True}):
            with self.assertRaises(ValueError):registry_tool.validate_remove_accents_profile(bad)
        for key,original in accent_profile.PROFILE.items():
            for value in (None,0.0,'different',not original if type(original) is bool else True):
                with self.subTest(key=key,value=value), self.assertRaises(ValueError):
                    registry_tool.validate_remove_accents_profile({**accent_profile.PROFILE,key:value})

    def test_remove_accents_coverage_requires_all_locale_classes_and_exact_actual_and_zero_ambient_calls(self):
        branches=dict.fromkeys(accent_profile.BRANCHES,1)
        registry_tool.validate_remove_accents_coverage(branches,6,0,6)
        registry_tool.validate_remove_accents_calls(0,0,0)
        for position,value in [(0,None),(0,{**branches,'base':True}),(0,{'base':6}),
            (0,{**branches,'bosnian':0}),(0,{**branches,'extra':1}),
            (1,True),(1,6.0),(1,5),(2,False),(2,1),(3,True),(3,0)]:
            args=[branches,6,0,6];args[position]=value
            with self.assertRaises(ValueError):registry_tool.validate_remove_accents_coverage(*args)

    def test_wp_rehash_profile_requires_explicit_default_empty_metadata_and_exact_types(self):
        registry_tool.validate_wp_rehash_profile(dict(wp_rehash.PROFILE))
        for invalid in (None, {}, {**wp_rehash.PROFILE, 'site_policy': True}):
            with self.assertRaises(ValueError): registry_tool.validate_wp_rehash_profile(invalid)
        for key, original in wp_rehash.PROFILE.items():
            for value in (None, 0.0, 'different', not original if type(original) is bool else True):
                with self.subTest(key=key,value=value), self.assertRaises(ValueError):
                    registry_tool.validate_wp_rehash_profile({**wp_rehash.PROFILE,key:value})

    def test_wp_rehash_coverage_requires_both_domains_all_costs_and_three_exact_hook_totals(self):
        costs=dict.fromkeys(map(str,range(4,32)),1)
        hooks=dict.fromkeys(wp_rehash.HOOKS,29)
        args=({'nonprefixed':1,'canonical_prefixed':28},costs,hooks,29)
        registry_tool.validate_wp_rehash_coverage(*args)
        registry_tool.validate_wp_rehash_hooks(dict.fromkeys(wp_rehash.HOOKS,0),0)
        for position,value in [(0,None),(0,{'nonprefixed':29}),(0,{'nonprefixed':True,'canonical_prefixed':28}),
            (1,{}),(1,{**costs,'12':True}),(1,{**costs,'31':0}),(1,{**costs,'32':1}),
            (2,None),(2,{'password_needs_rehash':29}),(2,{**hooks,'password_needs_rehash':29.0}),
            (2,{**hooks,'wp_hash_password_options':28}),(3,True),(3,0)]:
            changed=list(args);changed[position]=value
            with self.assertRaises(ValueError): registry_tool.validate_wp_rehash_coverage(*changed)

    def test_wp_bcrypt_profile_has_exact_provider_domain_and_public_dummy_identity(self):
        registry_tool.validate_wp_bcrypt_profile(dict(wp_bcrypt.PROFILE))
        self.assertEqual(wp_bcrypt.vectors_hash(), wp_bcrypt.VECTORS_SHA256)
        for invalid in (None, {}, {**wp_bcrypt.PROFILE, "legacy": True}):
            with self.assertRaises(ValueError):
                registry_tool.validate_wp_bcrypt_profile(invalid)
        for key in wp_bcrypt.PROFILE:
            for value in (None, 0.0, "different", True):
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    registry_tool.validate_wp_bcrypt_profile({**wp_bcrypt.PROFILE, key: value})

    def test_wp_bcrypt_coverage_requires_cost12_and_length_guard_and_typed_hook_count(self):
        args=({"4":8,"12":2},{"4":8,"12":2},{"within_4096":9,"over_4096":1},10,10)
        registry_tool.validate_wp_bcrypt_coverage(*args)
        for position, invalid in ((0,None),(0,{"4":10}),(0,{"4":8.0,"12":2}),(1,{"4":8,"12":True}),
                                  (2,{"within_4096":10,"over_4096":0}),(3,True),(3,10.0),(3,9),(4,True)):
            changed=list(args);changed[position]=invalid
            with self.assertRaises(ValueError):
                registry_tool.validate_wp_bcrypt_coverage(*changed)

    def test_empty_filter_escaping_profile_requires_utf8_and_exact_hook_names_and_types(self):
        expected = registry_tool.EMPTY_FILTER_ESCAPING_PROFILE
        registry_tool.validate_empty_filter_escaping_profile(dict(expected))
        for bad in (None, {}, {**expected, "dispatcher": True},
                    {key: value for key, value in expected.items() if key != "filter_registry"}):
            with self.assertRaises(ValueError):
                registry_tool.validate_empty_filter_escaping_profile(bad)
        for key in expected:
            for value in (None, 0.0, "different", 1):
                if type(value) is type(expected[key]) and value == expected[key]:
                    continue
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    registry_tool.validate_empty_filter_escaping_profile({**expected, key: value})

    def test_empty_filter_observed_hook_counts_match_each_api_without_esc_attr_alias(self):
        counts = {"esc_html": 2, "esc_attr": 3}
        registry_tool.validate_empty_filter_hook_counts({"esc_html": 2, "attribute_escape": 3}, counts)
        registry_tool.validate_empty_filter_hook_counts({"esc_html": 0, "attribute_escape": 0}, {})
        for bad in (None, {}, {"esc_html": 2, "esc_attr": 3}, {"esc_html": True, "attribute_escape": 3},
                    {"esc_html": 2.0, "attribute_escape": 3}, {"esc_html": 2, "attribute_escape": 2},
                    {"esc_html": 2, "attribute_escape": 3, "extra": 0}):
            with self.assertRaises(ValueError):
                registry_tool.validate_empty_filter_hook_counts(bad, counts)
        with self.assertRaises(ValueError):
            registry_tool.validate_empty_filter_hook_counts({"esc_html": 1, "attribute_escape": 3}, {"esc_html": True, "esc_attr": 3})
        with self.assertRaises(ValueError):
            registry_tool.validate_empty_filter_hook_counts({"esc_html": 0, "attribute_escape": 0}, None)

    def test_specialchars_profile_requires_typed_selected_flags_and_source_identity(self):
        expected = registry_tool.SPECIALCHARS_PROFILE
        registry_tool.validate_specialchars_profile(dict(expected))
        for bad in (None, {}, {**expected, "raw_flags": 19},
                    {key: value for key, value in expected.items() if key != "charset"}):
            with self.assertRaises(ValueError):
                registry_tool.validate_specialchars_profile(bad)
        for key in expected:
            for value in (None, True, False, 0.0, "different"):
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    registry_tool.validate_specialchars_profile({**expected, key: value})

    def test_specialchars_matrix_requires_all_twelve_positive_integer_cells_and_exact_total(self):
        expected = {style + ":" + double: 1 for style in registry_tool.SPECIALCHARS_STYLES for double in ("false", "true")}
        registry_tool.validate_specialchars_matrix(dict(expected), 12)
        for bad, total in ((None, 12), ({}, 12), ({**expected, "raw19:false": 1}, 13),
                           ({key: value for key, value in expected.items() if key != "xml1:true"}, 11),
                           ({**expected, "noquotes:false": 0}, 11),
                           ({**expected, "noquotes:false": True}, 12),
                           ({**expected, "noquotes:false": 1.0}, 12), (expected, True), (expected, 13)):
            with self.subTest(bad=bad, total=total), self.assertRaises(ValueError):
                registry_tool.validate_specialchars_matrix(bad, total)

    def test_xml_utf8_profile_and_both_context_counts_are_strict(self):
        expected = xml_entities.profile()
        registry_tool.validate_xml_entity_profile(dict(expected))
        for invalid in (None, {}, {**expected, "extra": True}):
            with self.assertRaises(ValueError):
                registry_tool.validate_xml_entity_profile(invalid)
        for key in expected:
            for value in (None, True, 253.0, "different"):
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    registry_tool.validate_xml_entity_profile({**expected, key: value})
        registry_tool.validate_entity_profile_counts({"html": 1, "xml": 2}, 3)
        for counts, total in (({"html": 3}, 3), ({"html": 1, "xml": 0}, 1),
                              ({"html": True, "xml": 2}, 3), ({"html": 1, "xml": 2.0}, 3),
                              ({"html": 1, "xml": 2}, True), ({"html": 1, "xml": 2}, 4)):
            with self.assertRaises(ValueError):
                registry_tool.validate_entity_profile_counts(counts, total)

    def test_xml_mapping_literal_pairs_and_generated_bytes_are_pinned(self):
        self.assertEqual(xml_entities.extract_xml_names("$allowedxmlentitynames = array('amp','lt','gt','apos','quot',);"), xml_entities.XML_NAMES)
        for source in ("", "$allowedxmlentitynames=array('amp',);",
                       "$allowedxmlentitynames=array('amp',helper(),);",
                       "$allowedxmlentitynames=array('amp','amp',);"):
            with self.assertRaises(ValueError):
                xml_entities.extract_xml_names(source)
        pairs = xml_entities.read_pairs()
        for bad in (pairs[:-1], list(reversed(pairs)), [*pairs[:-1], pairs[0]],
                    [[pairs[0][0], "FF"], *pairs[1:]], [[pairs[0][0], "00"], *pairs[1:]]):
            with self.assertRaises(ValueError):
                xml_entities.validate_pairs(bad)

    def test_protocol_outer_profile_is_typed_and_has_no_internal_top_guard(self):
        expected = registry_tool.PROTOCOL_OUTER_PROFILE
        registry_tool.validate_protocol_outer_profile(dict(expected))
        for invalid in (None, {}, {**expected, "extra": 1}):
            with self.assertRaises(ValueError):
                registry_tool.validate_protocol_outer_profile(invalid)
        for key in expected:
            for value in (None, 0, 1.0, "different", True):
                if type(value) is type(expected[key]) and value == expected[key]:
                    continue
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    registry_tool.validate_protocol_outer_profile({**expected, key: value})

    def test_protocol_once_profile_requires_default_count_c_locale_and_top_only_bound(self):
        expected = registry_tool.PROTOCOL_ONCE_PROFILE
        registry_tool.validate_protocol_once_profile(dict(expected))
        for invalid in (None, {}, {**expected, "recursive_input_limit": 1048576},
                        {key: value for key, value in expected.items() if key != "default_count"}):
            with self.assertRaises(ValueError):
                registry_tool.validate_protocol_once_profile(invalid)
        for key in expected:
            for value in (None, True, 1.0, "different", -1):
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    registry_tool.validate_protocol_once_profile({**expected, key: value})

    def test_byte_entity_decode_profile_requires_exact_typed_bounds_and_pass_order(self):
        expected = registry_tool.ENTITY_DECODE_PROFILE
        registry_tool.validate_entity_decode_profile(dict(expected))
        for invalid in (None, {}, {**expected, "recursive": True},
                        {key: value for key, value in expected.items() if key != "max_input_bytes"}):
            with self.assertRaises(ValueError):
                registry_tool.validate_entity_decode_profile(invalid)
        for key in expected:
            for value in (None, True, 255.0, "hex_then_decimal", -1):
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    registry_tool.validate_entity_decode_profile({**expected, key: value})

    def test_default_html_entity_profile_requires_exact_typed_source_and_name_identity(self):
        expected = entity_names.profile()
        registry_tool.validate_default_html_entity_profile(dict(expected))
        for invalid in (None, {}, {**expected, "xml": True},
                        {key: value for key, value in expected.items() if key != "source_sha256"}):
            with self.assertRaises(ValueError):
                registry_tool.validate_default_html_entity_profile(invalid)
        for key in expected:
            for value in (None, True, 253.0, "different"):
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    registry_tool.validate_default_html_entity_profile({**expected, key: value})

    def test_entity_literal_extraction_rejects_ambiguity_expressions_and_duplicate_names(self):
        good = "$allowedentitynames = array( 'amp', 'quot', );"
        self.assertEqual(entity_names.extract_names(good), ["amp", "quot"])
        for source in ("", good + good, "$allowedentitynames = array( 'amp', helper(), );",
                       "$allowedentitynames = array( 'amp', 'amp', );",
                       "$allowedentitynames = array( 'a', );",
                       "$allowedentitynames = array( 'amp' . 'quot', );",
                       "$allowedentitynames = array( 'amp', /* expression */ 'quot', );",
                       "$allowedentitynames = array( 'amp', "):
            with self.subTest(source=source), self.assertRaises(ValueError):
                entity_names.extract_names(source)

    def test_entity_data_check_requires_both_exact_generated_files(self):
        base = reference.ROOT / "target/entity-data-tests"
        base.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=base) as folder:
            root = Path(folder)
            paths = [root / path for path in (entity_names.ARTIFACT, entity_names.RUST_DATA)]
            for path, wire in zip(paths, ("artifact\n", "rust\n")):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(wire.encode())
            with patch.object(reference, "ROOT", root), patch.object(entity_names, "generated", return_value=("artifact\n", "rust\n")):
                entity_names.check()
                for path, wire in zip(paths, (b"artifact\n", b"rust\n")):
                    path.write_bytes(wire + b"stale")
                    with self.assertRaisesRegex(ValueError, "stale"):
                        entity_names.check()
                    path.write_bytes(wire)

    def test_byte_conversion_default_units_require_exact_integer_profile(self):
        expected = registry_tool.HR_BYTE_UNITS
        registry_tool.validate_hr_byte_units(dict(expected))
        for invalid in (None, {}, {**expected, "TB_IN_BYTES": 1024**4},
                        {key: value for key, value in expected.items() if key != "KB_IN_BYTES"}):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                registry_tool.validate_hr_byte_units(invalid)
        for key in expected:
            for value in (None, True, str(expected[key]), float(expected[key]), expected[key] + 1):
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    registry_tool.validate_hr_byte_units({**expected, key: value})
        registry_tool.validate_hr_byte_domain(256)
        for value in (None, True, "256", 256.0, 255, 257):
            with self.subTest(max_significant_digits=value), self.assertRaises(ValueError):
                registry_tool.validate_hr_byte_domain(value)

    def test_json_utf8_fallback_profile_requires_exact_charset_function_availability_and_types(self):
        expected = registry_tool.JSON_UTF8_FALLBACK_PROFILE
        registry_tool.validate_json_utf8_fallback_profile(dict(expected))
        for invalid in (None, {}, {**expected, "extra": True},
                        {**expected, "blog_charset": "latin1"},
                        {**expected, "mb_convert_encoding_exists": True},
                        {**expected, "mb_convert_encoding_exists": 0},
                        {**expected, "is_utf8_charset": 1},
                        {**expected, "is_utf8_charset": False}):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                registry_tool.validate_json_utf8_fallback_profile(invalid)

    def test_json_profile_coverage_requires_both_paths_and_exact_positive_counts(self):
        counts = {"first_encode_only": 3, "utf8_no_mbstring": 2}
        registry_tool.validate_json_profile_cases(counts, 5)
        for invalid in ({}, {"first_encode_only": 5}, {**counts, "other": 1},
                        {**counts, "utf8_no_mbstring": True}, {**counts, "utf8_no_mbstring": 2.0},
                        {**counts, "utf8_no_mbstring": 0}, {**counts, "utf8_no_mbstring": 3}):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                registry_tool.validate_json_profile_cases(invalid, 5)
        for total in (True, 5.0, 4):
            with self.assertRaises(ValueError):
                registry_tool.validate_json_profile_cases(counts, total)

    def test_json_flag_coverage_requires_each_supported_combination_and_exact_integer_counts(self):
        bits = list(registry_tool.JSON_ENCODE_FLAGS.values())
        counts = {str(sum(flag for offset, flag in enumerate(bits) if choice & (1 << offset))): 1
                  for choice in range(1 << len(bits))}
        registry_tool.validate_json_flag_cases(counts, len(counts))
        for invalid in ({}, {**counts, "32": 1}, {key: value for key, value in counts.items() if key != "2048"},
                        {**counts, "0": True}, {**counts, "0": 1.0}, {**counts, "0": 0}, {**counts, "0": 2}):
            with self.assertRaises(ValueError):
                registry_tool.validate_json_flag_cases(invalid, len(counts))
        for total in (True, float(len(counts)), len(counts)-1):
            with self.assertRaises(ValueError):
                registry_tool.validate_json_flag_cases(counts, total)

    def test_json_encoding_flag_constants_reject_type_confusion_and_missing_or_extra_bits(self):
        expected = registry_tool.JSON_ENCODE_FLAGS
        registry_tool.validate_json_encode_flags(dict(expected))
        for key in expected:
            for value in (None, True, float(expected[key]), expected[key] + 1):
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    registry_tool.validate_json_encode_flags({**expected, key: value})
        for flags in (None, {}, {**expected, "JSON_NUMERIC_CHECK": 32},
                      {key: value for key, value in expected.items() if key != "JSON_HEX_TAG"}):
            with self.assertRaises(ValueError):
                registry_tool.validate_json_encode_flags(flags)

    def test_stale_implementation_and_inconsistent_results_are_rejected(self):
        root = (reference.ROOT / "target" / "evidence-tests").resolve()
        assert root.is_relative_to(reference.ROOT.resolve())
        root.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(dir=root)
        assert Path(temporary.name).resolve().parent == root
        with temporary as folder:
            workspace = Path(folder)
            source = workspace / "lib.rs"
            source.write_text("implementation", encoding="utf-8")
            evidence = {
                "format_version": 1,
                "wordpress_version": reference.VERSION,
                "archive_sha256": "archive",
                "mismatches": 0,
                "case_count": 1,
                "jsonp_ctype_locale": "C",
                "php_mbstring_loaded": False,
                "utf8_oracle_path": "no_mbstring_fallback",
                "php_parse_str_settings": {"arg_separator.input":"&", "max_input_vars":"1000", "max_input_nesting_level":"64"},
                "case_counts_by_api": {"example": 1},
                "comparison_batches": 1,
                "batch_case_limit": 1,
                "batch_input_limit_bytes": 1024,
                "max_observed_batch_bytes": 100,
                "rust_results_sha256": "same",
                "wordpress_results_sha256": "same",
                "scope": "isolated_function_subsets_with_real_wp_filter_dispatch",
                "limitations": ["subset"],
                "implementation_files": {"lib.rs": reference.digest_file(source)},
                "harness_files": {"lib.rs": reference.digest_file(source)},
                "source_files": {},
            }
            registry = {"wordpress_version": reference.VERSION, "archive_sha256": "archive", "apis": [{"wordpress_symbol": "example", "differential_evidence": "evidence.json"}]}
            path = workspace / "evidence.json"
            path.write_text(json.dumps(evidence), encoding="utf-8")
            with patch.object(reference, "ROOT", workspace), patch.object(
                registry_tool, "evidence_files", return_value={"implementation_files": {"lib.rs"}, "harness_files": {"lib.rs"}}
            ), patch.object(registry_tool, "evidence_sources", return_value=set()):
                self.assertEqual(registry_tool.validate_evidence("evidence.json", registry)["case_count"], 1)
                source.write_text("changed implementation", encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "stale"):
                    registry_tool.validate_evidence("evidence.json", registry)
                source.write_text("implementation", encoding="utf-8")
                for field, value in [("jsonp_ctype_locale", "en_US.UTF-8"), ("jsonp_ctype_locale", False), ("php_parse_str_settings", {}), ("php_parse_str_settings", {"arg_separator.input":";&", "max_input_vars":"1000"}), ("format_version", True), ("php_mbstring_loaded", True), ("php_mbstring_loaded", 0), ("utf8_oracle_path", "mbstring"), ("mismatches", 1), ("mismatches", False), ("case_count", 2), ("case_count", True),
                                     ("case_counts_by_api", {"other": 1}), ("case_counts_by_api", {"example": True}),
                                     ("comparison_batches", 0), ("comparison_batches", 2), ("batch_case_limit", True), ("max_observed_batch_bytes", 1025),
                                     ("implementation_files", {}), ("harness_files", {}),
                                     ("implementation_files", {"other.rs": reference.digest_file(source)}),
                                     ("source_files", {"wp-includes/other.php": "unused"}),
                                     ("wordpress_results_sha256", "different"), ("scope", "full_compatibility"), ("wordpress_version", "other")]:
                    modified = dict(evidence)
                    modified[field] = value
                    path.write_text(json.dumps(modified), encoding="utf-8")
                    with self.subTest(field=field), self.assertRaises(ValueError):
                        registry_tool.validate_evidence("evidence.json", registry)
                with self.assertRaisesRegex(ValueError, "escaped"):
                    registry_tool.validate_evidence("../outside.json", registry)
                unique_registry = {**registry, "apis": [{"wordpress_symbol": "wp_unique_id", "differential_evidence": "evidence.json"}]}
                unique_evidence = {**evidence, "case_counts_by_api": {"wp_unique_id": 1},
                                   "unique_id_state_scope": registry_tool.UNIQUE_ID_STATE_SCOPE}
                path.write_text(json.dumps(unique_evidence), encoding="utf-8")
                registry_tool.validate_evidence("evidence.json", unique_registry)
                for lifetime in (None, False, "one_context_per_case", "process_global_forever"):
                    path.write_text(json.dumps({**unique_evidence, "unique_id_state_scope": lifetime}), encoding="utf-8")
                    with self.subTest(lifetime=lifetime), self.assertRaisesRegex(ValueError, "context lifetime"):
                        registry_tool.validate_evidence("evidence.json", unique_registry)
                byte_registry = {**registry, "apis": [{"wordpress_symbol": "wp_convert_hr_to_bytes", "differential_evidence": "evidence.json"}]}
                byte_evidence = {**evidence, "case_counts_by_api": {"wp_convert_hr_to_bytes": 1},
                                 "php_hr_byte_units": dict(registry_tool.HR_BYTE_UNITS),
                                 "php_hr_byte_max_significant_digits": 256}
                path.write_text(json.dumps(byte_evidence), encoding="utf-8")
                registry_tool.validate_evidence("evidence.json", byte_registry)
                for profile in (None, {}, {**registry_tool.HR_BYTE_UNITS, "KB_IN_BYTES": True},
                                {**registry_tool.HR_BYTE_UNITS, "GB_IN_BYTES": float(1024**3)}):
                    path.write_text(json.dumps({**byte_evidence, "php_hr_byte_units": profile}), encoding="utf-8")
                    with self.subTest(profile=profile), self.assertRaisesRegex(ValueError, "unit profile"):
                        registry_tool.validate_evidence("evidence.json", byte_registry)
                for digits in (None, True, 256.0, "256", 257):
                    path.write_text(json.dumps({**byte_evidence, "php_hr_byte_max_significant_digits": digits}), encoding="utf-8")
                    with self.subTest(digits=digits), self.assertRaisesRegex(ValueError, "significant digit"):
                        registry_tool.validate_evidence("evidence.json", byte_registry)
                del byte_evidence["php_hr_byte_units"]
                path.write_text(json.dumps(byte_evidence), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "unit profile"):
                    registry_tool.validate_evidence("evidence.json", byte_registry)
                entity_registry = {**registry, "apis": [{"wordpress_symbol": "wp_kses_normalize_entities", "differential_evidence": "evidence.json"}]}
                entity_evidence = {**evidence, "case_count": 2, "case_counts_by_api": {"wp_kses_normalize_entities": 2},
                                   "php_default_html_entity_profile": entity_names.profile(),
                                   "php_xml_utf8_entity_profile": xml_entities.profile(),
                                   "entity_context_case_counts": {"html": 1, "xml": 1}}
                path.write_text(json.dumps(entity_evidence), encoding="utf-8")
                registry_tool.validate_evidence("evidence.json", entity_registry)
                for profile in (None, {}, {**entity_names.profile(), "names_count": True},
                                {**entity_names.profile(), "context": "xml"}):
                    path.write_text(json.dumps({**entity_evidence, "php_default_html_entity_profile": profile}), encoding="utf-8")
                    with self.assertRaisesRegex(ValueError, "entity profile"):
                        registry_tool.validate_evidence("evidence.json", entity_registry)
                for field, invalid in (("php_xml_utf8_entity_profile", None),
                                       ("php_xml_utf8_entity_profile", {**xml_entities.profile(), "html5_flag": 48.0}),
                                       ("entity_context_case_counts", None),
                                       ("entity_context_case_counts", {"html": 2}),
                                       ("entity_context_case_counts", {"html": True, "xml": 1})):
                    path.write_text(json.dumps({**entity_evidence, field: invalid}), encoding="utf-8")
                    with self.subTest(field=field, invalid=invalid), self.assertRaises(ValueError):
                        registry_tool.validate_evidence("evidence.json", entity_registry)
                decode_registry = {**registry, "apis": [{"wordpress_symbol": "wp_kses_decode_entities", "differential_evidence": "evidence.json"}]}
                special_registry = {**registry, "apis": [{"wordpress_symbol": "_wp_specialchars", "differential_evidence": "evidence.json"}]}
                special_evidence = {**evidence, "case_count": 12, "case_counts_by_api": {"_wp_specialchars": 12},
                                    "php_specialchars_utf8_profile": dict(registry_tool.SPECIALCHARS_PROFILE),
                                    "specialchars_style_double_case_counts": {style + ":" + double: 1 for style in registry_tool.SPECIALCHARS_STYLES for double in ("false", "true")}}
                path.write_text(json.dumps(special_evidence), encoding="utf-8")
                registry_tool.validate_evidence("evidence.json", special_registry)
                for field in ("php_specialchars_utf8_profile", "specialchars_style_double_case_counts"):
                    path.write_text(json.dumps({key: value for key, value in special_evidence.items() if key != field}), encoding="utf-8")
                    with self.assertRaises(ValueError):
                        registry_tool.validate_evidence("evidence.json", special_registry)
                escaping_registry = {**registry, "apis": [{"wordpress_symbol": api, "differential_evidence": "evidence.json"} for api in ("esc_html", "esc_attr")]}
                bcrypt_registry = {**registry, "apis": [{"wordpress_symbol": "wp_check_password", "differential_evidence": "evidence.json"}]}
                rehash_registry={**registry,'apis':[{'wordpress_symbol':'wp_password_needs_rehash','differential_evidence':'evidence.json'}]}
                accent_registry={**registry,'apis':[{'wordpress_symbol':'remove_accents','differential_evidence':'evidence.json'}]}
                strip_registry={**registry,'apis':[{'wordpress_symbol':'wp_strip_all_tags','differential_evidence':'evidence.json'}]}
                strip_evidence={**evidence,'case_count':2,'case_counts_by_api':{'wp_strip_all_tags':2},'php_strip_all_tags_profile':dict(user_profile.STRIP_PROFILE),
                    'strip_all_tags_reference_calls':2,'strip_all_tags_flag_case_counts':{'false':1,'true':1}}
                path.write_text(json.dumps(strip_evidence),encoding='utf-8');registry_tool.validate_evidence('evidence.json',strip_registry)
                for field in ('php_strip_all_tags_profile','strip_all_tags_reference_calls','strip_all_tags_flag_case_counts'):
                    path.write_text(json.dumps({key:value for key,value in strip_evidence.items()if key!=field}),encoding='utf-8')
                    with self.assertRaises(ValueError):registry_tool.validate_evidence('evidence.json',strip_registry)
                user_registry={**registry,'apis':[{'wordpress_symbol':'sanitize_user','differential_evidence':'evidence.json'}]}
                user_evidence={**evidence,'case_count':2,'case_counts_by_api':{'sanitize_user':2},'php_sanitize_user_profile':dict(user_profile.USER_PROFILE),
                    'sanitize_user_reference_calls':2,'sanitize_user_filter_calls':2,'sanitize_user_locale_hook_counts':{'zero':1,'one':1},
                    'sanitize_user_expected_locale_hook_counts':{'zero':1,'one':1},'locale_db_branch_calls':0,
                    'sanitize_user_strict_case_counts':{'false':1,'true':1},'previous_api66_input_sha256':user_profile.ACCENT_INPUT_SHA256}
                path.write_text(json.dumps(user_evidence),encoding='utf-8');registry_tool.validate_evidence('evidence.json',user_registry)
                for field in ('php_sanitize_user_profile','sanitize_user_reference_calls','sanitize_user_filter_calls','sanitize_user_locale_hook_counts',
                    'sanitize_user_expected_locale_hook_counts','locale_db_branch_calls','sanitize_user_strict_case_counts','previous_api66_input_sha256'):
                    path.write_text(json.dumps({key:value for key,value in user_evidence.items()if key!=field}),encoding='utf-8')
                    with self.assertRaises(ValueError):registry_tool.validate_evidence('evidence.json',user_registry)
                accent_evidence={**evidence,'case_count':6,'case_counts_by_api':{'remove_accents':6},
                    'php_remove_accents_profile':dict(accent_profile.PROFILE),'remove_accents_locale_branch_case_counts':dict.fromkeys(accent_profile.BRANCHES,1),
                    'remove_accents_reference_calls':6,'remove_accents_get_locale_calls':0}
                truncate_registry={**registry,'apis':[{'wordpress_symbol':'_truncate_post_slug','differential_evidence':'evidence.json'}]}
                truncate_evidence={**evidence,'case_count':3,'case_counts_by_api':{'_truncate_post_slug':3},
                    'php_truncate_slug_profile':dict(truncate_profile.PROFILE),'truncate_slug_reference_calls':3,
                    'truncate_slug_branch_counts':dict.fromkeys(truncate_profile.BRANCHES,1),
                    'truncate_slug_expected_branch_counts':dict.fromkeys(truncate_profile.BRANCHES,1),
                    'previous_api71_input_sha256':truncate_profile.OLD_INPUT_SHA256,'previous_api71_results_sha256':truncate_profile.OLD_OUTPUT_SHA256}
                path.write_text(json.dumps(truncate_evidence),encoding='utf-8');registry_tool.validate_evidence('evidence.json',truncate_registry)
                for missing in ('php_truncate_slug_profile','truncate_slug_reference_calls','truncate_slug_branch_counts',
                    'truncate_slug_expected_branch_counts','previous_api71_input_sha256','previous_api71_results_sha256'):
                    path.write_text(json.dumps({key:value for key,value in truncate_evidence.items()if key!=missing}),encoding='utf-8')
                    with self.subTest(missing=missing),self.assertRaises(ValueError):registry_tool.validate_evidence('evidence.json',truncate_registry)
                default_registry={**registry,'apis':[{'wordpress_symbol':'sanitize_title','differential_evidence':'evidence.json'}]}
                default_evidence={**evidence,'case_count':3,'case_counts_by_api':{'sanitize_title':3},
                    'php_default_sanitize_title_profile':dict(default_title_profile.PROFILE),'default_title_reference_calls':3,
                    'default_title_filter_calls':3,'default_title_locale_hook_counts':{'zero':2,'one':1},
                    'default_title_expected_locale_hook_counts':{'zero':2,'one':1},'default_title_global_restorations':3,
                    'default_title_context_case_counts':{'save':1,'display':1,'query':1},
                    'previous_api70_input_sha256':default_title_profile.OLD_INPUT_SHA256,'previous_api70_results_sha256':default_title_profile.OLD_OUTPUT_SHA256}
                path.write_text(json.dumps(default_evidence),encoding='utf-8');registry_tool.validate_evidence('evidence.json',default_registry)
                for missing in ('php_default_sanitize_title_profile','default_title_reference_calls','default_title_filter_calls',
                    'default_title_locale_hook_counts','default_title_expected_locale_hook_counts','default_title_global_restorations',
                    'default_title_context_case_counts','previous_api70_input_sha256','previous_api70_results_sha256'):
                    path.write_text(json.dumps({key:value for key,value in default_evidence.items()if key!=missing}),encoding='utf-8')
                    with self.subTest(missing=missing),self.assertRaises(ValueError):registry_tool.validate_evidence('evidence.json',default_registry)
                for api,title in (('utf8_uri_encode',False),('sanitize_title_with_dashes',True)):
                    total=2 if title else 4
                    prefix='title_dash'if title else'utf8_uri'
                    coverage={'display':1,'save':1}if title else{'zero:false':1,'zero:true':1,'positive:false':1,'positive:true':1}
                    field=prefix+('_context_case_counts'if title else'_option_case_counts')
                    values={**evidence,'case_count':total,'case_counts_by_api':{api:total},
                        'php_'+prefix+'_profile':dict(slug_profile.TITLE_PROFILE if title else slug_profile.URI_PROFILE),
                        prefix+'_reference_calls':total,field:coverage,
                        'previous_api68_input_sha256':slug_profile.OLD_INPUT_SHA256,'previous_api68_results_sha256':slug_profile.OLD_OUTPUT_SHA256}
                    selected={**registry,'apis':[{'wordpress_symbol':api,'differential_evidence':'evidence.json'}]}
                    path.write_text(json.dumps(values),encoding='utf-8');registry_tool.validate_evidence('evidence.json',selected)
                    for missing in ('php_'+prefix+'_profile',prefix+'_reference_calls',field,'previous_api68_input_sha256','previous_api68_results_sha256'):
                        path.write_text(json.dumps({key:value for key,value in values.items()if key!=missing}),encoding='utf-8')
                        with self.subTest(missing=missing),self.assertRaises(ValueError):registry_tool.validate_evidence('evidence.json',selected)
                path.write_text(json.dumps(accent_evidence),encoding='utf-8')
                registry_tool.validate_evidence('evidence.json',accent_registry)
                for field in ('php_remove_accents_profile','remove_accents_locale_branch_case_counts','remove_accents_reference_calls','remove_accents_get_locale_calls'):
                    path.write_text(json.dumps({key:value for key,value in accent_evidence.items()if key!=field}),encoding='utf-8')
                    with self.assertRaises(ValueError):registry_tool.validate_evidence('evidence.json',accent_registry)
                rehash_evidence={**evidence,'case_count':29,'case_counts_by_api':{'wp_password_needs_rehash':29},
                    'php_wp_rehash_profile':dict(wp_rehash.PROFILE),'wp_rehash_case_kind_counts':{'nonprefixed':1,'canonical_prefixed':28},
                    'wp_rehash_cost_case_counts':dict.fromkeys(map(str,range(4,32)),1),'wp_rehash_hook_counts':dict.fromkeys(wp_rehash.HOOKS,29)}
                path.write_text(json.dumps(rehash_evidence),encoding='utf-8')
                registry_tool.validate_evidence('evidence.json',rehash_registry)
                for field in ('php_wp_rehash_profile','wp_rehash_case_kind_counts','wp_rehash_cost_case_counts','wp_rehash_hook_counts'):
                    path.write_text(json.dumps({key:value for key,value in rehash_evidence.items() if key!=field}),encoding='utf-8')
                    with self.assertRaises(ValueError):registry_tool.validate_evidence('evidence.json',rehash_registry)
                bcrypt_evidence = {**evidence, "case_count":10,"case_counts_by_api":{"wp_check_password":10},
                    "php_wp_bcrypt_profile":dict(wp_bcrypt.PROFILE),"wp_bcrypt_hash_cost_case_counts":{"4":8,"12":2},
                    "wp_bcrypt_host_max_cost_case_counts":{"4":8,"12":2},"wp_bcrypt_length_case_counts":{"within_4096":9,"over_4096":1},"wp_bcrypt_check_password_calls":10}
                path.write_text(json.dumps(bcrypt_evidence),encoding="utf-8")
                registry_tool.validate_evidence("evidence.json",bcrypt_registry)
                for field in ("php_wp_bcrypt_profile","wp_bcrypt_hash_cost_case_counts","wp_bcrypt_host_max_cost_case_counts","wp_bcrypt_length_case_counts","wp_bcrypt_check_password_calls"):
                    path.write_text(json.dumps({key:value for key,value in bcrypt_evidence.items() if key!=field}),encoding="utf-8")
                    with self.assertRaises(ValueError):registry_tool.validate_evidence("evidence.json",bcrypt_registry)
                escaping_evidence = {**evidence, "case_count": 2, "case_counts_by_api": {"esc_html": 1, "esc_attr": 1},
                                    "php_empty_filter_escaping_profile": dict(registry_tool.EMPTY_FILTER_ESCAPING_PROFILE),
                                    "empty_filter_escaping_hook_counts": {"esc_html": 1, "attribute_escape": 1}}
                path.write_text(json.dumps(escaping_evidence), encoding="utf-8")
                registry_tool.validate_evidence("evidence.json", escaping_registry)
                for field, invalid in (("php_empty_filter_escaping_profile", None),
                                       ("php_empty_filter_escaping_profile", {**registry_tool.EMPTY_FILTER_ESCAPING_PROFILE, "attr_hook": "esc_attr"}),
                                       ("empty_filter_escaping_hook_counts", None),
                                       ("empty_filter_escaping_hook_counts", {"esc_html": True, "attribute_escape": 1})):
                    path.write_text(json.dumps({**escaping_evidence, field: invalid}), encoding="utf-8")
                    with self.assertRaises(ValueError):
                        registry_tool.validate_evidence("evidence.json", escaping_registry)
                decode_evidence = {**evidence, "case_counts_by_api": {"wp_kses_decode_entities": 1},
                                   "php_entity_decode_profile": dict(registry_tool.ENTITY_DECODE_PROFILE)}
                path.write_text(json.dumps(decode_evidence), encoding="utf-8")
                registry_tool.validate_evidence("evidence.json", decode_registry)
                protocol_registry = {**registry, "apis": [{"wordpress_symbol": "wp_kses_bad_protocol_once2", "differential_evidence": "evidence.json"}]}
                protocol_evidence = {**decode_evidence, "case_counts_by_api": {"wp_kses_bad_protocol_once2": 1}}
                path.write_text(json.dumps(protocol_evidence), encoding="utf-8")
                registry_tool.validate_evidence("evidence.json", protocol_registry)
                path.write_text(json.dumps({**protocol_evidence, "php_entity_decode_profile": None}), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "decoder profile"):
                    registry_tool.validate_evidence("evidence.json", protocol_registry)
                once_registry = {**registry, "apis": [{"wordpress_symbol": "wp_kses_bad_protocol_once", "differential_evidence": "evidence.json"}]}
                once_evidence = {**protocol_evidence, "case_counts_by_api": {"wp_kses_bad_protocol_once": 1},
                                 "php_protocol_once_profile": dict(registry_tool.PROTOCOL_ONCE_PROFILE)}
                path.write_text(json.dumps(once_evidence), encoding="utf-8")
                registry_tool.validate_evidence("evidence.json", once_registry)
                for profile in (None, {}, {**registry_tool.PROTOCOL_ONCE_PROFILE, "default_count": True},
                                {**registry_tool.PROTOCOL_ONCE_PROFILE, "ctype_locale": "en_US"}):
                    path.write_text(json.dumps({**once_evidence, "php_protocol_once_profile": profile}), encoding="utf-8")
                    with self.assertRaisesRegex(ValueError, "Protocol-once profile"):
                        registry_tool.validate_evidence("evidence.json", once_registry)
                outer_registry = {**registry, "apis": [{"wordpress_symbol": "wp_kses_bad_protocol", "differential_evidence": "evidence.json"}]}
                outer_evidence = {**decode_evidence, "case_counts_by_api": {"wp_kses_bad_protocol": 1},
                                  "php_protocol_outer_profile": dict(registry_tool.PROTOCOL_OUTER_PROFILE)}
                path.write_text(json.dumps(outer_evidence), encoding="utf-8")
                registry_tool.validate_evidence("evidence.json", outer_registry)
                for profile in (None, {}, {**registry_tool.PROTOCOL_OUTER_PROFILE, "max_once_passes": True},
                                {**registry_tool.PROTOCOL_OUTER_PROFILE, "internal_top_guard": 0}):
                    path.write_text(json.dumps({**outer_evidence, "php_protocol_outer_profile": profile}), encoding="utf-8")
                    with self.assertRaisesRegex(ValueError, "Protocol-outer profile"):
                        registry_tool.validate_evidence("evidence.json", outer_registry)
                for profile in (None, {}, {**registry_tool.ENTITY_DECODE_PROFILE, "max_reference_value": True},
                                {**registry_tool.ENTITY_DECODE_PROFILE, "max_input_bytes": 1048576.0},
                                {**registry_tool.ENTITY_DECODE_PROFILE, "order": "hex_then_decimal"}):
                    path.write_text(json.dumps({**decode_evidence, "php_entity_decode_profile": profile}), encoding="utf-8")
                    with self.assertRaisesRegex(ValueError, "decoder profile"):
                        registry_tool.validate_evidence("evidence.json", decode_registry)

    def test_required_files_cover_modules_helpers_and_build_inputs(self):
        for scope in (registry_tool.PRIMITIVE_SCOPE, registry_tool.HOOK_SCOPE, registry_tool.DISPATCHER_SCOPE):
            groups = registry_tool.evidence_files(scope)
            paths = set().union(*groups.values())
            for required in ("crates/recastpress-wp/src/filtered.rs", "crates/recastpress-wp/src/dispatcher.rs",
                             "crates/recastpress-wp/examples/common/mod.rs", "scripts/php_value_codec.php",
                             "Cargo.toml", "Cargo.lock", "crates/recastpress-wp/Cargo.toml"):
                with self.subTest(scope=scope, required=required):
                    self.assertIn(required, paths)
                    self.assertTrue((reference.ROOT / required).is_file())

    def test_duplicate_json_keys_are_rejected_at_any_depth(self):
        for wire in ('{"case_count":1,"case_count":2}', '{"files":{"a":"old","a":"new"}}'):
            with self.subTest(wire=wire), self.assertRaisesRegex(ValueError, "duplicate"):
                json.loads(wire, object_pairs_hook=registry_tool.unique_object)


if __name__ == "__main__":
    unittest.main()
