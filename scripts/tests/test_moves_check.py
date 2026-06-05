"""T2 R2, R3, R5, R8: self-tests for moves_check.py, on fixtures and on the real base sources."""

import re
import subprocess
import sys
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "scripts"))

import moves_check  # noqa: E402
from moves_check import FAMILIES, compare_sources, guard_paths, parse_source, tokenize  # noqa: E402

ROOT = "src/x.rs"
BASE = """use std::fmt;
use std::sync::Arc;

// --- Types ---

pub struct X {
    value: u32,
    pub name: String,
}

struct Private;

use std::collections::HashMap;

impl X {
    /// Creates a value.
    pub fn new(value: u32, name: String) -> Self {
        // Keep the limit small.
        let limit = 10;
        let label = format!("{} name", name);
        Self { value: value + limit, name: label }
    }

    // --- Readers ---

    pub fn get(&self) -> Result<u32, String> {
        let first = self.check()?;
        let second = self.value;
        match first < second {
            true => Ok(first),
            false => Ok(second),
        }
    }

    fn check(&self) -> Result<u32, String> {
        Ok(self.value)
    }

    pub async fn fetch(&self, first: u32, second: &str) -> u32 {
        use std::cmp::max;
        let got = remote(second).await;
        let top = self.items().map(|v| v + 1).filter(|v| *v > 2).sum();
        max(max(got, first), top)
    }

    #[inline]
    fn items(&self) -> std::vec::IntoIter<u32> {
        vec![self.value].into_iter()
    }

    pub fn other(&self) -> u32 {
        self.value
    }
}

impl fmt::Display for X {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

async fn remote(_: &str) -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_test() {
        assert_eq!(1, 1);
    }

    #[test]
    fn second_test() {
        assert_eq!(2, 2);
    }
}
"""

CHECK = """    fn check(&self) -> Result<u32, String> {
        Ok(self.value)
    }

"""
OTHER = """    pub fn other(&self) -> u32 {
        self.value
    }
"""
DISPLAY = """impl fmt::Display for X {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

"""


def sub(text, old, new):
    """Replace, and refuse to continue if the fixture no longer holds the text."""
    if old not in text:
        raise AssertionError(f"fixture does not contain: {old!r}")
    return text.replace(old, new, 1)


def child_with_check(header="use super::X;\n\nimpl X {\n"):
    return header + "    pub(super) fn check(&self) -> Result<u32, String> {\n        Ok(self.value)\n    }\n}\n"


def moved_check(child_name="reader"):
    """The base with X::check moved to a declared child."""
    parent = "mod " + child_name + ";\n" + sub(BASE, CHECK, "")
    return {ROOT: parent, f"src/x/{child_name}.rs": child_with_check()}


class Case(unittest.TestCase):
    def findings(self, current, base=BASE):
        return compare_sources({ROOT: base}, current, "fixture")

    def assert_clean(self, current, base=BASE):
        self.assertEqual(self.findings(current, base), [])

    def assert_finding(self, current, base=BASE, contains=None):
        found = self.findings(current, base)
        self.assertTrue(found, "expected a finding")
        if contains:
            self.assertTrue(any(contains in f for f in found), f"{contains!r} not in {found}")

    def assert_edit_fails(self, old, new):
        self.assert_finding({ROOT: sub(BASE, old, new)})


class Tokenizer(unittest.TestCase):
    def texts(self, source):
        return [token.text for token in tokenize(source)]

    def test_lifetime_is_not_a_char_literal(self):
        self.assertEqual(self.texts("&'a str"), ["&", "'a", "str"])
        self.assertEqual(self.texts("fn f<'a, 'b>()")[2:7], ["<", "'a", ",", "'b", ">"])

    def test_char_literals_are_single_tokens(self):
        self.assertEqual(self.texts(r"""['a', '\'', '"', '\\', b'x']""")[1::2],
                         ["'a'", r"'\''", "'\"'", r"'\\'", "b'x'"])

    def test_raw_and_byte_strings_are_single_tokens(self):
        source = r'''r#"a "quoted" b"# b"x\"y" "s\"t" br"z" c"w"'''
        self.assertEqual(
            self.texts(source), [r'r#"a "quoted" b"#', r'b"x\"y"', r'"s\"t"', 'br"z"', 'c"w"']
        )

    def test_nested_block_comment_is_one_token(self):
        self.assertEqual(self.texts("a /* b /* c */ d */ e"), ["a", "/* b /* c */ d */", "e"])

    def test_comment_keeps_inner_but_not_trailing_whitespace(self):
        self.assertEqual(self.texts("// a  b   \nc"), ["// a  b", "c"])

    def test_numbers_and_ranges(self):
        self.assertEqual(self.texts("0..10"), ["0", ".", ".", "10"])
        self.assertEqual(self.texts("1.5e-3f32 0xFF_u8"), ["1.5e-3f32", "0xFF_u8"])

    def test_raw_identifier(self):
        self.assertEqual(self.texts("r#type"), ["r#type"])

    def test_punctuation_is_one_token_per_character(self):
        self.assertEqual(self.texts("a::b<=c"), ["a", ":", ":", "b", "<", "=", "c"])


class MustPass(Case):
    def test_unchanged(self):
        self.assert_clean({ROOT: BASE})

    def test_impl_split_over_two_files(self):
        self.assert_clean(moved_check())

    def test_signature_rewrapped_by_pub_super_with_trailing_comma(self):
        parent = "mod fetcher;\n" + sub(
            BASE,
            "    fn check(&self) -> Result<u32, String> {\n        Ok(self.value)\n    }\n\n",
            "",
        )
        child = (
            "use super::X;\n\nimpl X {\n"
            "    pub(super) fn check(\n        &self,\n    ) -> Result<u32, String> {\n"
            "        Ok(self.value)\n    }\n}\n"
        )
        self.assert_clean({ROOT: parent, "src/x/fetcher.rs": child})

    def test_generic_return_rewrapped_with_trailing_comma_before_angle(self):
        current = sub(
            BASE,
            "pub fn get(&self) -> Result<u32, String> {",
            "pub fn get(\n        &self,\n    ) -> Result<\n        u32,\n        String,\n    > {",
        )
        self.assert_clean({ROOT: current})

    def test_reflowed_method_chain(self):
        self.assert_clean(
            {
                ROOT: sub(
                    BASE,
                    "self.items().map(|v| v + 1).filter(|v| *v > 2).sum();",
                    "self\n            .items()\n            .map(|v| v + 1)\n"
                    "            .filter(|v| *v > 2)\n            .sum();",
                )
            }
        )

    def test_mid_file_use_removed(self):
        self.assert_clean({ROOT: sub(BASE, "use std::collections::HashMap;\n", "")})

    def test_mid_file_use_moved(self):
        current = sub(BASE, "use std::collections::HashMap;\n", "")
        self.assert_clean({ROOT: "use std::collections::HashMap;\n" + current})

    def test_mod_and_named_pub_use(self):
        parent = sub(BASE, "struct Private;\n", "")
        parent = "mod hidden;\npub use hidden::Private;\n" + parent
        self.assert_clean(
            {ROOT: parent, "src/x/hidden.rs": "struct Private;\n"},
        )

    def test_mod_and_braced_pub_use(self):
        parent = sub(BASE, "struct Private;\n", "")
        parent = "pub(super) mod hidden;\npub use hidden::{Private, Public};\n" + parent
        self.assert_clean({ROOT: parent, "src/x/hidden.rs": "struct Private;\n"})

    def test_whole_trait_impl_moved(self):
        parent = "mod display;\n" + sub(BASE, DISPLAY, "")
        self.assert_clean({ROOT: parent, "src/x/display.rs": "use std::fmt;\n\n" + DISPLAY})

    def test_field_widened_to_pub_super(self):
        self.assert_clean({ROOT: sub(BASE, "    value: u32,", "    pub(super) value: u32,")})

    def test_field_widened_to_pub_crate(self):
        self.assert_clean({ROOT: sub(BASE, "    value: u32,", "    pub(crate) value: u32,")})

    def test_members_reordered_across_files(self):
        parent = sub(BASE, CHECK, "")
        parent = sub(parent, OTHER, "")
        parent = sub(parent, "    pub fn get(&self)", OTHER + "\n    pub fn get(&self)")
        self.assert_clean({"src/x.rs": "mod reader;\n" + parent, "src/x/reader.rs": child_with_check()})

    def test_section_comment_stays_in_parent_while_members_move(self):
        self.assertIn("// --- Readers ---", moved_check()[ROOT])
        self.assert_clean(moved_check())

    def test_section_comment_moves_with_members(self):
        parent = "mod reader;\n" + sub(sub(BASE, CHECK, ""), "    // --- Readers ---\n\n", "")
        child = "use super::X;\n\nimpl X {\n    // --- Readers ---\n\n" + child_with_check("")
        self.assert_clean({ROOT: parent, "src/x/reader.rs": child})

    def test_impl_super_path_for_impl(self):
        current = moved_check()
        current["src/x/reader.rs"] = child_with_check("impl super::X {\n")
        self.assert_clean(current)

    def test_impl_super_super_path_in_nested_child(self):
        parent = "mod reader;\n" + sub(BASE, CHECK, "")
        self.assert_clean(
            {
                ROOT: parent,
                "src/x/reader.rs": "mod deep;\n",
                "src/x/reader/deep.rs": child_with_check("impl super::super::X {\n"),
            }
        )

    def test_use_added_inside_inline_test_module(self):
        self.assert_clean(
            {ROOT: sub(BASE, "    use super::*;\n", "    use super::*;\n    use std::mem;\n")}
        )

    def test_nested_child_declared_from_child_file(self):
        parent = "mod reader;\n" + sub(BASE, CHECK, "")
        self.assert_clean(
            {
                ROOT: parent,
                "src/x/reader.rs": "mod deep;\n",
                "src/x/reader/deep.rs": child_with_check("use super::super::X;\n\nimpl X {\n"),
            }
        )

    def test_child_in_mod_rs_form(self):
        parent = "mod reader;\n" + sub(BASE, CHECK, "")
        self.assert_clean({ROOT: parent, "src/x/reader/mod.rs": child_with_check()})

    def test_grandchild_of_mod_rs_form(self):
        parent = "mod reader;\n" + sub(BASE, CHECK, "")
        self.assert_clean(
            {
                ROOT: parent,
                "src/x/reader/mod.rs": "mod deep;\n",
                "src/x/reader/deep.rs": child_with_check(),
            }
        )

    def test_mod_inside_inline_module_of_the_base(self):
        parent = sub(BASE, CHECK, "")
        parent = sub(parent, "    use super::*;\n", "    use super::*;\n    mod helper_data;\n")
        parent = "mod reader;\n" + parent
        self.assert_clean(
            {
                ROOT: parent,
                "src/x/reader.rs": child_with_check(),
                "src/x/tests/helper_data.rs": "",
            }
        )


class MustFailReachability(Case):
    def test_member_moved_to_undeclared_file(self):
        current = moved_check()
        current[ROOT] = sub(current[ROOT], "mod reader;\n", "")
        self.assert_finding(current, contains="src/x/reader.rs: no `mod` declares")

    def test_file_declared_only_from_undeclared_file(self):
        current = {
            ROOT: sub(BASE, CHECK, ""),
            "src/x/reader.rs": "mod deep;\n",
            "src/x/reader/deep.rs": child_with_check(),
        }
        self.assert_finding(current, contains="src/x/reader/deep.rs: no `mod` declares")

    def test_mod_only_in_a_line_comment(self):
        current = moved_check()
        current[ROOT] = sub(current[ROOT], "mod reader;\n", "")
        # A comment changes the bag too, so the reachability finding is asserted by name.
        current[ROOT] = "// mod reader;\n" + current[ROOT]
        self.assert_finding(current, contains="no `mod` declares")

    def test_mod_only_in_a_block_comment_inside_a_body(self):
        current = moved_check()
        current[ROOT] = sub(current[ROOT], "mod reader;\n", "")
        current[ROOT] = sub(current[ROOT], "let limit = 10;", "let limit = 10; /* mod reader; */")
        self.assert_finding(current, contains="no `mod` declares")

    def test_mod_only_in_a_string_literal(self):
        current = moved_check()
        current[ROOT] = sub(current[ROOT], "mod reader;\n", "")
        current[ROOT] = current[ROOT] + 'const FAKE: &str = "mod reader;";\n'
        self.assert_finding(current, contains="no `mod` declares")

    def test_file_at_a_path_its_mod_does_not_resolve_to(self):
        current = moved_check()
        current["src/x/sub/reader.rs"] = current.pop("src/x/reader.rs")
        self.assert_finding(current, contains="resolves to no family file")

    def test_file_beside_the_root_instead_of_under_it(self):
        current = moved_check()
        current["src/reader.rs"] = current.pop("src/x/reader.rs")
        self.assert_finding(current, contains="src/reader.rs")

    def test_inline_mod_absent_from_base_holding_a_mod(self):
        parent = sub(BASE, CHECK, "")
        parent = parent + "#[cfg(any())]\nmod a {\n    mod y;\n}\n"
        self.assert_finding({ROOT: parent, "src/x/a/y.rs": child_with_check()})

    def test_plain_inline_mod_absent_from_base_holding_a_mod(self):
        parent = sub(BASE, CHECK, "")
        parent = parent + "mod a {\n    mod y;\n}\n"
        self.assert_finding({ROOT: parent, "src/x/a/y.rs": child_with_check()})

    def test_mod_inside_a_function_body_is_no_declaration(self):
        parent = sub(BASE, CHECK, "")
        parent = sub(parent, "let limit = 10;", "mod reader;\n        let limit = 10;")
        self.assert_finding({ROOT: parent, "src/x/reader.rs": child_with_check()})


class MustFailBodies(Case):
    def test_changed_string_literal(self):
        self.assert_edit_fails('"{} name"', '"{} title"')

    def test_whitespace_only_change_inside_string_literal(self):
        self.assert_edit_fails('"{} name"', '"{}  name"')

    def test_changed_number(self):
        self.assert_edit_fails("let limit = 10;", "let limit = 11;")

    def test_less_than_to_less_or_equal(self):
        self.assert_edit_fails("first < second", "first <= second")

    def test_removed_question_mark(self):
        self.assert_edit_fails("self.check()?;", "self.check();")

    def test_removed_await(self):
        self.assert_edit_fails("remote(second).await;", "remote(second);")

    def test_added_exclamation_mark(self):
        self.assert_edit_fails("match first < second", "match !(first < second)")

    def test_added_single_bang(self):
        self.assert_edit_fails("assert_eq!(1, 1);", "assert_eq!(1, 1); !true;")

    def test_two_statements_swapped(self):
        self.assert_edit_fails(
            "let first = self.check()?;\n        let second = self.value;",
            "let second = self.value;\n        let first = self.check()?;",
        )

    def test_two_match_arms_swapped(self):
        self.assert_edit_fails(
            "true => Ok(first),\n            false => Ok(second),",
            "false => Ok(second),\n            true => Ok(first),",
        )

    def test_statement_added(self):
        self.assert_edit_fails("let limit = 10;", "let limit = 10;\n        drop(1);")

    def test_statement_removed(self):
        self.assert_edit_fails("        let second = self.value;\n", "")

    def test_statement_moved_between_functions(self):
        current = sub(BASE, "        let limit = 10;\n", "")
        current = sub(current, "        let second = self.value;\n", "        let second = self.value;\n        let limit = 10;\n")
        self.assert_finding({ROOT: current})

    def test_edited_comment_inside_body(self):
        self.assert_edit_fails("// Keep the limit small.", "// Keep the limit tiny.")

    def test_removed_comment_inside_body(self):
        self.assert_edit_fails("        // Keep the limit small.\n", "")

    def test_added_comment_inside_body(self):
        self.assert_edit_fails("let limit = 10;", "// New.\n        let limit = 10;")

    def test_super_path_newly_written_in_body(self):
        self.assert_edit_fails("remote(second).await", "super::remote(second).await")

    def test_self_path_newly_written_in_body(self):
        self.assert_edit_fails("remote(second).await", "self::remote(second).await")

    def test_changed_use_inside_function_body(self):
        self.assert_edit_fails("use std::cmp::max;", "use std::cmp::min as max;")

    def test_removed_use_inside_function_body(self):
        self.assert_edit_fails("        use std::cmp::max;\n", "")

    def test_helper_extracted_from_body(self):
        current = sub(BASE, "        let limit = 10;\n", "        let limit = Self::limit();\n")
        current = sub(current, "    // --- Readers ---\n", "    fn limit() -> u32 {\n        10\n    }\n\n    // --- Readers ---\n")
        self.assert_finding({ROOT: current})


class MustFailItems(Case):
    def test_renamed_function(self):
        self.assert_edit_fails("pub fn other(&self)", "pub fn another(&self)")

    def test_renamed_parameter(self):
        self.assert_edit_fails("fn new(value: u32, name: String)", "fn new(value: u32, title: String)")

    def test_reordered_parameters(self):
        self.assert_edit_fails("fetch(&self, first: u32, second: &str)", "fetch(&self, second: &str, first: u32)")

    def test_changed_type_in_signature(self):
        self.assert_edit_fails("pub fn other(&self) -> u32", "pub fn other(&self) -> u64")

    def test_new_empty_fn(self):
        self.assert_edit_fails("    // --- Readers ---\n", "    fn nothing() {}\n\n    // --- Readers ---\n")

    def test_new_empty_module_level_fn(self):
        self.assert_finding({ROOT: BASE + "fn nothing() {}\n"})

    def test_new_const(self):
        self.assert_finding({ROOT: BASE + "const NEW: u32 = 1;\n"})

    def test_new_struct(self):
        self.assert_finding({ROOT: BASE + "struct New;\n"})

    def test_new_struct_in_child(self):
        current = moved_check()
        current["src/x/reader.rs"] += "struct New {}\n"
        self.assert_finding(current)

    def test_added_allow_on_member(self):
        self.assert_edit_fails("    fn check(&self)", "    #[allow(dead_code)]\n    fn check(&self)")

    def test_added_inline_on_member(self):
        self.assert_edit_fails("    fn check(&self)", "    #[inline]\n    fn check(&self)")

    def test_added_cfg_test_on_member(self):
        self.assert_edit_fails("    fn check(&self)", "    #[cfg(test)]\n    fn check(&self)")

    def test_removed_attribute_on_member(self):
        self.assert_edit_fails("    #[inline]\n", "")

    def test_edited_doc_comment_on_member(self):
        self.assert_edit_fails("/// Creates a value.", "/// Creates an X.")

    def test_edited_section_comment(self):
        self.assert_edit_fails("// --- Readers ---", "// --- Getters ---")

    def test_removed_section_comment(self):
        self.assert_edit_fails("    // --- Readers ---\n", "")

    def test_added_section_comment(self):
        self.assert_edit_fails("    // --- Readers ---\n", "    // --- Readers ---\n    // --- More ---\n")

    def test_added_module_level_comment_in_child(self):
        current = moved_check()
        current["src/x/reader.rs"] = "// Reader methods.\n" + current["src/x/reader.rs"]
        self.assert_finding(current)

    def test_added_module_doc(self):
        self.assert_finding({ROOT: "//! The module.\n" + BASE})

    def test_added_module_doc_in_child(self):
        current = moved_check()
        current["src/x/reader.rs"] = "//! Reader methods.\n" + current["src/x/reader.rs"]
        self.assert_finding(current)

    def test_private_fn_made_pub(self):
        self.assert_edit_fails("    fn check(&self)", "    pub fn check(&self)")

    def test_private_field_made_pub(self):
        self.assert_edit_fails("    value: u32,", "    pub value: u32,")

    def test_private_struct_made_pub(self):
        self.assert_edit_fails("struct Private;", "pub struct Private;")

    def test_pub_fn_narrowed_to_pub_crate(self):
        self.assert_edit_fails("pub fn other(&self)", "pub(crate) fn other(&self)")

    def test_pub_fn_narrowed_to_pub_super(self):
        self.assert_edit_fails("pub fn other(&self)", "pub(super) fn other(&self)")

    def test_pub_field_narrowed_to_pub_crate(self):
        self.assert_edit_fails("    pub name: String,", "    pub(crate) name: String,")

    def test_pub_in_path_is_not_an_allowed_widening(self):
        self.assert_edit_fails("    fn check(&self)", "    pub(in crate) fn check(&self)")

    def test_private_inline_module_made_pub(self):
        self.assert_edit_fails("mod tests {", "pub mod tests {")

    def test_changed_impl_header(self):
        self.assert_edit_fails("impl fmt::Display for X {", "impl fmt::Debug for X {")

    def test_added_attribute_on_impl_header(self):
        self.assert_edit_fails("impl fmt::Display for X {", "#[cfg(test)]\nimpl fmt::Display for X {")

    def test_child_impl_with_a_changed_header(self):
        current = moved_check()
        current["src/x/reader.rs"] = child_with_check("impl<T> super::X {\n")
        self.assert_finding(current)

    def test_removed_test_in_inline_test_module(self):
        self.assert_edit_fails(
            "    #[test]\n    fn second_test() {\n        assert_eq!(2, 2);\n    }\n", ""
        )

    def test_edited_test_in_inline_test_module(self):
        self.assert_edit_fails("assert_eq!(2, 2);", "assert_eq!(2, 3);")

    def test_removed_whole_item(self):
        self.assert_edit_fails("struct Private;\n", "")

    def test_removed_empty_marker_impl(self):
        base = BASE + "unsafe impl Send for X {}\n"
        self.assert_finding({ROOT: BASE}, base=base)

    def test_added_empty_marker_impl(self):
        self.assert_finding({ROOT: BASE + "unsafe impl Send for X {}\n"})


class MustFailDeclarations(Case):
    def declared(self, line):
        current = moved_check()
        current[ROOT] = sub(current[ROOT], "mod reader;\n", line)
        return current

    def test_pub_mod(self):
        self.assert_finding(self.declared("pub mod reader;\n"), contains="reader")

    def test_pub_use_glob(self):
        current = moved_check()
        current[ROOT] = "pub use reader::*;\n" + current[ROOT]
        self.assert_finding(current)

    def test_pub_use_of_something_else(self):
        current = moved_check()
        current[ROOT] = "pub use std::fmt::Debug;\n" + current[ROOT]
        self.assert_finding(current)

    def test_pub_use_of_undeclared_child(self):
        current = moved_check()
        current[ROOT] = "pub use other::Name;\n" + current[ROOT]
        self.assert_finding(current, contains="needs `mod other;`")

    def test_pub_use_with_rename(self):
        current = moved_check()
        current[ROOT] = "pub use reader::Name as Other;\n" + current[ROOT]
        self.assert_finding(current)

    def test_pub_use_of_nested_path(self):
        current = moved_check()
        current[ROOT] = "pub use reader::deep::Name;\n" + current[ROOT]
        self.assert_finding(current)

    def test_pub_use_of_self_child_path(self):
        current = moved_check()
        current[ROOT] = "pub use self::reader::Name;\n" + current[ROOT]
        self.assert_finding(current)

    def test_mod_under_path_attribute(self):
        self.assert_finding(self.declared('#[path = "x/reader.rs"]\nmod reader;\n'))

    def test_mod_under_cfg_attribute(self):
        self.assert_finding(self.declared("#[cfg(test)]\nmod reader;\n"))

    def test_mod_under_cfg_any_attribute(self):
        self.assert_finding(self.declared("#[cfg(any())]\nmod reader;\n"))

    def test_mod_with_a_doc_comment(self):
        self.assert_finding(self.declared("/// Readers.\nmod reader;\n"))

    def test_inner_allow_in_child_file(self):
        current = moved_check()
        current["src/x/reader.rs"] = "#![allow(dead_code)]\n" + current["src/x/reader.rs"]
        self.assert_finding(current)

    def test_inner_cfg_in_child_file(self):
        current = moved_check()
        current["src/x/reader.rs"] = "#![cfg(any())]\n" + current["src/x/reader.rs"]
        self.assert_finding(current)

    def test_inner_allow_in_root(self):
        self.assert_finding({ROOT: "#![allow(dead_code)]\n" + BASE})

    def test_use_with_another_attribute(self):
        self.assert_finding({ROOT: "#[allow(unused_imports)]\nuse std::mem;\n" + BASE})

    def test_use_with_cfg_attribute_is_dropped(self):
        self.assert_clean({ROOT: "#[cfg(test)]\nuse std::mem;\n" + BASE})

    def test_pub_super_use_is_dropped(self):
        self.assert_clean({ROOT: "pub(super) use std::mem;\n" + BASE})


class FindingText(Case):
    def test_names_family_file_line_and_first_differing_token(self):
        current = moved_check()
        current["src/x/reader.rs"] = current["src/x/reader.rs"].replace("self.value", "self.name")
        found = compare_sources({ROOT: BASE}, current, "myfamily")
        self.assertEqual(len(found), 1, found)
        self.assertIn("[myfamily] src/x/reader.rs:5:", found[0])
        self.assertIn("'name'", found[0])
        self.assertIn("'value'", found[0])


class TreeGuard(unittest.TestCase):
    def test_allowed_paths(self):
        self.assertEqual(
            guard_paths(
                [
                    *FAMILIES.values(),
                    "nexus-player-backend/src/state/state/library.rs",
                    "crates/streaming/src/buffers/hls/deep/mod.rs",
                    "scripts/anything",
                    "nexus-player-backend/tests/public_paths.rs",
                    "crates/streaming/tests/public_paths.rs",
                ]
            ),
            [],
        )

    def test_protected_paths(self):
        for path in (
            "Cargo.toml",
            "Cargo.lock",
            "nexus-player-backend/Cargo.toml",
            "crates/streaming/Cargo.toml",
            "nexus-player-frontend/src-tauri/src/commands.rs",
            "nexus-player-frontend/src/App.vue",
            "crates/streaming/examples/demo.rs",
            "nexus-player-backend/src/db/migrations/m1.rs",
            "README.md",
            "LICENSE",
            "CLAUDE.md",
            "vendor/x/lib.rs",
            "nexus-player-backend/src/state/mod.rs",
            "crates/streaming/src/buffers/mod.rs",
            "nexus-player-backend/src/library/service.rs",
            "nexus-player-backend/tests/other.rs",
            "crates/streaming/tests/other.rs",
            "nexus-player-backend/src/state/state/notes.md",
            "nexus-player-backend/src/state/states.rs",
        ):
            with self.subTest(path=path):
                self.assertTrue(guard_paths([path]), path)


class BaseSha(unittest.TestCase):
    @staticmethod
    def fake_git(failing):
        class Done:
            def __init__(self, code):
                self.returncode = code

        def git(*args, check=True):
            del check
            return Done(1 if args[0] == failing else 0)

        return git

    def test_garbage_is_rejected(self):
        for text in ("", "main", "abc123", "A" * 40):
            with self.subTest(text=text):
                self.assertTrue(moves_check.base_sha_problems(text))

    def test_valid_sha_passes_when_self_lookup_fails(self):
        calls = []

        class Done:
            def __init__(self, code):
                self.returncode = code

        def git(*args, check=True):
            del check
            calls.append(args)
            is_self_lookup = args[0] == "cat-file" and args[2].endswith(moves_check.BASE_FILE)
            return Done(1 if is_self_lookup else 0)

        self.assertEqual(moves_check.base_sha_problems("a" * 40, git), [])
        self.assertEqual(len(calls), 3)

    def test_commit_holding_the_base_file_is_rejected(self):
        class Done:
            returncode = 0

        self.assertTrue(moves_check.base_sha_problems("a" * 40, lambda *a, **k: Done()))

    def test_non_ancestor_is_rejected(self):
        self.assertTrue(moves_check.base_sha_problems("a" * 40, self.fake_git("merge-base")))

    def test_unknown_commit_is_rejected(self):
        self.assertTrue(moves_check.base_sha_problems("a" * 40, self.fake_git("cat-file")))

    def test_real_base_file(self):
        text = (REPO / moves_check.BASE_FILE).read_text(encoding="utf-8")
        self.assertEqual(moves_check.base_sha_problems(text), [])


def base_text(path):
    sha = (REPO / moves_check.BASE_FILE).read_text(encoding="utf-8").strip()
    return subprocess.run(
        ["git", "show", f"{sha}:{path}"], cwd=REPO, capture_output=True, check=True
    ).stdout.decode("utf-8")


FN_LINE = re.compile(r"^    (?:pub )?(?:const )?(?:async )?(?:unsafe )?fn ", re.M)
STATEMENT_LINE = re.compile(r"^ {8,}[a-z_][^\n]*;\n", re.M)


class RealSources(unittest.TestCase):
    def test_each_base_file_equals_itself(self):
        for family, root in FAMILIES.items():
            with self.subTest(family=family):
                text = base_text(root)
                self.assertEqual(compare_sources({root: text}, {root: text}, family), [])

    def test_fn_member_segments_match_four_space_fn_lines(self):
        for family, root in FAMILIES.items():
            with self.subTest(family=family):
                text = base_text(root)
                members = [
                    s for s in parse_source(root, text).segments if s.kind == "fn" and s.owner
                ]
                self.assertEqual(len(members), len(FN_LINE.findall(text)))
                self.assertGreater(len(members), 5)

    def test_mutating_the_first_string_literal_is_a_finding(self):
        for family, root in FAMILIES.items():
            with self.subTest(family=family):
                text = base_text(root)
                match = re.search(r'(?<![\'\w])"[^"\\\n]*"', text)
                mutated = text[: match.start() + 1] + "X" + text[match.start() + 1 :]
                self.assertTrue(compare_sources({root: text}, {root: mutated}, family))

    def test_whitespace_inside_the_first_string_literal_is_a_finding(self):
        for family, root in FAMILIES.items():
            with self.subTest(family=family):
                text = base_text(root)
                match = re.search(r'(?<![\'\w])"[^"\\\n]*"', text)
                mutated = text[: match.end() - 1] + " " + text[match.end() - 1 :]
                self.assertTrue(compare_sources({root: text}, {root: mutated}, family))

    def test_deleting_one_statement_is_a_finding(self):
        for family, root in FAMILIES.items():
            with self.subTest(family=family):
                text = base_text(root)
                match = STATEMENT_LINE.search(text)
                mutated = text[: match.start()] + text[match.end() :]
                self.assertTrue(compare_sources({root: text}, {root: mutated}, family))

    def test_base_holds_no_body_path_the_move_could_need(self):
        # The premise of not normalizing super:: and self:: in bodies: at the base they occur
        # only on module-level `use` lines, which the comparison drops.
        for family, root in FAMILIES.items():
            with self.subTest(family=family):
                text = base_text(root)
                for segment in parse_source(root, text).segments:
                    texts = [token.text for token in segment.tokens]
                    for index, word in enumerate(texts):
                        if word in ("super", "self"):
                            self.assertNotEqual(texts[index + 1 : index + 2], [":"], texts[:8])


if __name__ == "__main__":
    unittest.main()
