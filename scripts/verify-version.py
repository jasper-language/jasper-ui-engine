#!/usr/bin/env python3
"""Verify unchanged production UI function bodies and retain scoped evidence.
验证未改写的 UI 生产函数体，保留限定范围的可复核证据。"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys


# Contracts are specifications, not alternate executable implementations.
# 契约只描述规格；可执行函数体始终从生产源码读取。
KERNELS = {
    "ui-update-next": {
        "source":"src/input_identity.rs", "marker":"UI UPDATE NEXT", "property":"UI-UPDATE-NEXT-001",
        "signature":"pub fn next_ui_update_version(current: u64, expected: u64) -> Option<u64> {",
        "contract":"""pub fn next_ui_update_version(current: u64, expected: u64) -> (result: Option<u64>)
    ensures match result {
        Some(next) => current == expected && current < u64::MAX && next as int == current as int + 1,
        None => current != expected || current == u64::MAX,
    },
{""",
        "depends":[], "evidence":"ui-update-next-kernel", "function_count":1,
        "scope":"actual next-version preparation used by native component dispatch; mismatch/exhaustion rejection and exact successor only; no backend commit/rollback proof",
        "mutations":[("accept-wrong-sequence","if current != expected || current == u64::MAX {","if current == u64::MAX {"),("skip-version-increment","Some(current + 1)","Some(current)")],
    },

    "ui-snapshot-admission": {
        "source":"src/input_identity.rs", "marker":"UI SNAPSHOT ADMISSION", "property":"UI-SNAPSHOT-ADMISSION-001",
        "signature":"pub fn accepts_ui_display_snapshot(", "contract":"pub fn accepts_ui_display_snapshot(",
        "extra_annotations":[(") -> bool {", """ ) -> (result: bool)
    ensures result == (session == caller && current.target.0 == expected.target.0
        && current.target_generation == expected.target_generation
        && current.document == expected.document && current.revision == expected.revision
        && next.target.0 == expected.target.0 && next.target_generation == expected.target_generation
        && next.document == expected.document),
{""")],
        "depends":["ui-view-owner"], "evidence":"ui-snapshot-admission-kernel", "function_count":1,
        "scope":"actual host display snapshot admission predicate used by native adapter; opaque next revision unconstrained; not backend mutation atomicity, authentication or revision allocator",
        "mutations":[("allow-document-switch","        && next.document == expected.document\n","        && true\n")],
    },

    "ui-view-owner": {
        "source":"src/input_identity.rs", "marker":"UI VIEW OWNER", "property":"UI-VIEW-OWNER-001",
        "signature":"pub fn matches_ui_view_owner(",
        "contract":"pub fn matches_ui_view_owner(",
        "extra_annotations":[(") -> bool {", """ ) -> (result: bool)
    ensures result == (session == caller && current.target.0 == incoming.target.0
        && current.target_generation == incoming.target_generation
        && current.document == incoming.document && current.revision == incoming.revision),
{""")],
        "depends":["ui-input-admission"], "evidence":"ui-view-owner-kernel", "function_count":1,
        "scope":"actual five-dimensional view owner predicate used by native preedit bridge; not authentication, binding uniqueness, native mutation or external cancellation",
        "mutations":[("ignore-owner-session","    session == caller\n","    true\n"),("ignore-view-generation","        && current.target.0 == incoming.target.0\n        && current.target_generation == incoming.target_generation\n","        && current.target.0 == incoming.target.0\n        && true\n")],
    },

    "ui-view-write-borrow": {
        "source": "src/native_adapter/view_slot.rs", "marker": "UI VIEW WRITE BORROW", "property": "UI-VIEW-WRITE-001",
        "signature": "pub fn as_mut(&mut self) -> Option<&mut T> {",
        "contract": """pub fn as_mut(&mut self) -> (result: Option<&mut T>)
    ensures match result {
        Some(value) => old(self).owned_value() == Some(*value)
            && final(self).owned_value() == Some(*final(value)),
        None => old(self).owned_value().is_none() && final(self).owned_value().is_none(),
    },
{""",
        "depends": ["ui-view-slot"], "evidence": "ui-view-write-borrow-kernel",
        "scope": "actual mutable borrow value correspondence before and after the borrow; not physical pointer identity, native destructor or whole adapter",
        "function_count": 1,
        "mutations": [("drop-live-mutable-reference", "self.value.as_mut()", "None")],
    },

    "ui-view-read-borrow": {
        "source": "src/native_adapter/view_slot.rs", "marker": "UI VIEW READ BORROW", "property": "UI-VIEW-READ-001",
        "signature": "pub fn as_ref(&self) -> Option<&T> {",
        "contract": """pub fn as_ref(&self) -> (result: Option<&T>)
    ensures match result {
        Some(value) => self.owned_value() == Some(*value),
        None => self.owned_value().is_none(),
    },
{""",
        "depends": ["ui-view-slot"], "evidence": "ui-view-read-borrow-kernel",
        "scope": "exact production read-borrow value/occupancy correspondence over generic T, with owned-slot lifecycle dependencies; not physical pointer identity, as_mut, native destructor effects or whole adapter",
        "function_count": 1,
        "mutations": [("drop-live-reference", "self.value.as_ref()", "None")],
    },

    "ui-view-slot": {
        "source": "src/native_adapter/view_slot.rs", "marker": "UI VIEW SLOT", "property": "UI-VIEW-LIFETIME-001",
        "signature": "pub fn new(value: T) -> Self {",
        "contract": """pub fn new(value: T) -> (result: Self)
    ensures result.owned_value() == Some(value),
{""",
        "extra_annotations": [
            ("pub fn close(&mut self) -> bool {", """pub fn close(&mut self) -> (result: bool)
    ensures final(self).owned_value().is_none(), result == old(self).owned_value().is_some(),
{"""),
            ("pub fn is_open(&self) -> bool {", """pub fn is_open(&self) -> (result: bool)
    ensures result == self.owned_value().is_some(),
{"""),
        ],
        "spec_prelude": "impl<T> ViewSlot<T> { pub closed spec fn owned_value(&self) -> Option<T> { self.value } }",
        "function_count": 3, "depends": [], "evidence": "ui-view-slot-kernel",
        "scope": "exact generic owned-slot new/close/is_open bodies used by native UI facade; logical retirement only, not native destructor effects, as_ref/as_mut, host or callback cancellation",
        "mutations": [("retain-live-value", "self.value.take().is_some()", "self.value.is_some()"), ("wrong-open-result", "self.value.is_some()\n    }", "self.value.is_none()\n    }")],
    },

    "text-hit-offset": {
        "source": "experiments/deferred-skia-integration/text_skia.rs", "marker": "TEXT HIT OFFSET", "property": "UI-LTF-09-OFFSET",
        "signature": "fn text_hit_offset(position: i32, utf16_len: usize) -> Option<usize> {",
        "contract": """fn text_hit_offset(position: i32, utf16_len: usize) -> (result: Option<usize>)
    ensures match result {
        Some(offset) => position >= 0 && offset as int == position as int && offset <= utf16_len,
        None => position < 0 || position as int > utf16_len as int,
    },
{""",
        "depends": [], "evidence": "text-hit-offset-kernel",
        "scope": "actual native hit signed-position to bounded UTF16 offset admission only; not coordinate lookup, affinity, grapheme boundaries, edit authorization or full renderer",
        "mutations": [("admit-negative", "if position < 0 {", "if false {"), ("drop-upper-bound", "if offset > utf16_len {", "if false {")],
    },
    "local-text-storage": {
        "source": "experiments/deferred-skia-integration/text_skia.rs", "marker": "LOCAL TEXT STORAGE", "property": "UI-LTF-01",
        "signature": "fn text_frame_storage(width: u32, height: u32, max_bytes: usize) -> Option<(usize, usize)> {",
        "contract": """fn text_frame_storage(width: u32, height: u32, max_bytes: usize) -> (result: Option<(usize, usize)>)
    ensures match result {
        Some((row, end)) => width > 0 && height > 0 && width <= 2147483647 && height <= 2147483647
            && row as int == width as int * 4 && end <= max_bytes
            && end as int == (height as int - 1) * row as int + row as int,
        None => true,
    },
{""",
        "depends": ["surface-row", "surface-span"], "evidence": "local-text-storage-kernel",
        "scope": "actual local text frame storage admission soundness including native dimension range; existing row/span bodies reverified; not native rendering, allocation, float layout or whole adapter",
        "mutations": [("wrong-output-span", "Some(end) => Some((row, end))", "Some(end) => Some((row, row))"),
                      ("drop-native-dimension-bound", "if width > i32::MAX as u32 || height > i32::MAX as u32", "if false")],
    },
    "version": {
        "source": "src/scene.rs", "marker": "VERSION", "property": "UI-TXN-VERSION-001",
        "signature": "fn next_scene_version(version: u64) -> Option<u64> {",
        "contract": """fn next_scene_version(version: u64) -> (result: Option<u64>)
    ensures
        match result {
            Some(next) => version < u64::MAX
                && next as int == version as int + 1 && next > version,
            None => version == u64::MAX,
        },
{""",
        "depends": [], "evidence": "version-kernel",
        "scope": "next_scene_version body only; all u64 inputs; not the whole scene transaction",
        "mutations": [("old-version", "Some(version + 1)", "Some(version)")],
    },
    "decode-range": {
        "source": "src/batch.rs", "marker": "DECODE RANGE", "property": "UI-DECODE-RANGE-001-END",
        "signature": "fn checked_read_end(total: usize, pos: usize, len: usize) -> Option<usize> {",
        "contract": """fn checked_read_end(total: usize, pos: usize, len: usize) -> (result: Option<usize>)
    ensures
        match result {
            Some(end) => pos <= end && end <= total
                && end as int == pos as int + len as int,
            None => pos > total || pos as int + len as int > total as int,
        },
{""",
        "depends": [], "evidence": "decode-range-kernel",
        "scope": "checked_read_end body only; all usize inputs; not the full decoder",
        "mutations": [("wrong-end", "Some(pos + len)", "Some(pos)")],
    },
    "transaction-gate": {
        "source": "src/scene.rs", "marker": "TRANSACTION GATE", "property": "UI-TXN-ADMISSION-001",
        "signature": "fn decide_scene_version(current: u64, expected: u64, empty: bool) -> VersionDecision {",
        "contract": """fn decide_scene_version(current: u64, expected: u64, empty: bool) -> (result: VersionDecision)
    ensures
        match result {
            VersionDecision::Stale => expected != current,
            VersionDecision::Unchanged => expected == current && empty,
            VersionDecision::Exhausted => expected == current && !empty && current == u64::MAX,
            VersionDecision::Advance(next) => expected == current && !empty
                && current < u64::MAX && next as int == current as int + 1,
        },
{""",
        "depends": ["version"], "evidence": "transaction-gate-kernel",
        "scope": "decide_scene_version and its verified dependency; not the full scene transaction",
        "mutations": [("wrong-admission", "if expected != current", "if expected == current")],
    },
    "byte-read": {
        "source": "src/batch.rs", "marker": "BYTE READ", "property": "UI-DECODE-READ-001",
        "signature": "fn take_bytes<'a>(bytes: &'a [u8], pos: &mut usize, len: usize) -> Option<&'a [u8]> {",
        "contract": """fn take_bytes<'a>(bytes: &'a [u8], pos: &mut usize, len: usize) -> (result: Option<&'a [u8]>)
    ensures
        match result {
            Some(slice) => *old(pos) <= *final(pos) && *final(pos) <= bytes.len()
                && *final(pos) as int == *old(pos) as int + len as int
                && slice@ == bytes@.subrange(*old(pos) as int, *final(pos) as int)
                && slice.len() == len,
            None => *final(pos) == *old(pos)
                && (*old(pos) > bytes.len()
                    || *old(pos) as int + len as int > bytes.len() as int),
        },
{""",
        "depends": ["decode-range"], "evidence": "byte-read-kernel",
        "scope": "take_bytes exact slice and cursor state plus verified endpoint dependency; not full UiBatch decoding",
        "mutations": [
            ("no-advance", "*pos = end;", "*pos = *pos;"),
            ("failed-read-moves", "None => None,", "None => { *pos = 0; None },"),
        ],
    },
    "batch-error-type": {
        "source": "src/batch.rs", "marker": "BATCH ERROR", "suffix": "TYPE",
        "depends": [], "signature": None, "internal_only": True,
    },
    "cursor-read": {
        "source": "src/batch.rs", "marker": "CURSOR READ", "property": "UI-DECODE-RANGE-001",
        "signature": "fn take(&mut self, len: usize) -> Result<&'a [u8], BatchError> {",
        "contract": """fn take(&mut self, len: usize) -> (result: Result<&'a [u8], BatchError>)
    ensures
        final(self).bytes@ == old(self).bytes@,
        match result {
            Ok(slice) => old(self).pos <= final(self).pos
                && final(self).pos <= old(self).bytes.len()
                && final(self).pos as int == old(self).pos as int + len as int
                && slice@ == old(self).bytes@.subrange(old(self).pos as int, final(self).pos as int)
                && slice.len() == len,
            Err(error) => final(self).pos == old(self).pos
                && (old(self).pos > old(self).bytes.len()
                    || old(self).pos as int + len as int > old(self).bytes.len() as int)
                && matches!(error, BatchError::Truncated),
        },
{""",
        "depends": ["batch-error-type", "byte-read"], "evidence": "cursor-read-kernel",
        "scope": "Cursor::take exact bytes, cursor frame and Truncated result with verified read dependencies; not composite fields or full decoder",
        "mutations": [
            ("reject-success", "Some(slice) => Ok(slice),", "Some(slice) => Err(BatchError::Truncated),"),
            ("wrong-error", "None => Err(BatchError::Truncated),", "None => Err(BatchError::BadMagic),"),
        ],
    },
    "scalar-read": {
        "source": "src/batch.rs", "marker": "SCALAR READ", "property": "UI-DECODE-SCALAR-001",
        "signature": "fn u8(&mut self) -> Result<u8, BatchError> {",
        "contract": """fn u8(&mut self) -> (result: Result<u8, BatchError>)
    ensures
        final(self).bytes@ == old(self).bytes@,
        match result {
            Ok(value) => old(self).pos < old(self).bytes.len()
                && final(self).pos as int == old(self).pos as int + 1
                && value == old(self).bytes@[old(self).pos as int],
            Err(error) => final(self).pos == old(self).pos
                && old(self).pos >= old(self).bytes.len()
                && matches!(error, BatchError::Truncated),
        },
{""",
        "extra_annotations": [(
            "fn u16(&mut self) -> Result<u16, BatchError> {",
            """fn u16(&mut self) -> (result: Result<u16, BatchError>)
    ensures
        final(self).bytes@ == old(self).bytes@,
        match result {
            Ok(value) => old(self).pos as int + 2 <= old(self).bytes.len()
                && final(self).pos as int == old(self).pos as int + 2
                && value as int == old(self).bytes@[old(self).pos as int] as int
                    + 256 * old(self).bytes@[old(self).pos as int + 1] as int,
            Err(error) => final(self).pos == old(self).pos
                && (old(self).pos > old(self).bytes.len()
                    || old(self).pos as int + 2 > old(self).bytes.len() as int)
                && matches!(error, BatchError::Truncated),
        },
{""",
        ), (
            "fn u32(&mut self) -> Result<u32, BatchError> {",
            """fn u32(&mut self) -> (result: Result<u32, BatchError>)
    ensures
        final(self).bytes@ == old(self).bytes@,
        match result {
            Ok(value) => old(self).pos as int + 4 <= old(self).bytes.len()
                && final(self).pos as int == old(self).pos as int + 4
                && value as int == old(self).bytes@[old(self).pos as int] as int
                    + 256 * old(self).bytes@[old(self).pos as int + 1] as int
                    + 65536 * old(self).bytes@[old(self).pos as int + 2] as int
                    + 16777216 * old(self).bytes@[old(self).pos as int + 3] as int,
            Err(error) => final(self).pos == old(self).pos
                && (old(self).pos > old(self).bytes.len()
                    || old(self).pos as int + 4 > old(self).bytes.len() as int)
                && matches!(error, BatchError::Truncated),
        },
{""",
        )],
        "depends": ["cursor-read"], "evidence": "scalar-read-kernel", "function_count": 3,
        "scope": "Cursor u8/u16/u32 exact values and state with verified read dependencies; not floats, strings or full decoder",
        "mutations": [
            ("wrong-byte", "Ok(self.take(1)?[0])", "Ok(0)"),
            ("wrong-endian", "bytes[0] as u32", "bytes[1] as u32"),
        ],
    },
    "header-constants": {
        "source": "src/batch.rs", "marker": "HEADER CONSTANTS", "suffix": "TYPE",
        "depends": [], "signature": None, "internal_only": True,
    },
    "header-read": {
        "source": "src/batch.rs", "marker": "HEADER READ", "property": "UI-DECODE-HEADER-001",
        "signature": "fn decode_header(bytes: &[u8]) -> Result<u32, BatchError> {",
        "spec_prelude": """
pub open spec fn header_magic(s: Seq<u8>) -> bool {
    s.len() >= 10 && s[0] == 74 && s[1] == 85 && s[2] == 69 && s[3] == 66
}
pub open spec fn header_version(s: Seq<u8>) -> int recommends s.len() >= 10 {
    s[4] as int + 256 * s[5] as int
}
pub open spec fn header_count(s: Seq<u8>) -> int recommends s.len() >= 10 {
    s[6] as int + 256 * s[7] as int + 65536 * s[8] as int + 16777216 * s[9] as int
}
""",
        "contract": """fn decode_header(bytes: &[u8]) -> (result: Result<u32, BatchError>)
    ensures
        match result {
            Ok(count) => bytes.len() >= 10 && header_magic(bytes@)
                && header_version(bytes@) == VERSION as int
                && count as int == header_count(bytes@)
                && count <= MAX_OPS && count as int <= bytes.len() as int - 10,
            Err(BatchError::BadMagic) => bytes.len() >= 10 && !header_magic(bytes@),
            Err(BatchError::UnsupportedVersion(v)) => bytes.len() >= 10 && header_magic(bytes@)
                && header_version(bytes@) != VERSION as int && v as int == header_version(bytes@),
            Err(BatchError::TooLarge) => bytes.len() >= 10 && header_magic(bytes@)
                && header_version(bytes@) == VERSION as int && header_count(bytes@) > MAX_OPS as int,
            Err(BatchError::Truncated) => bytes.len() < 10 || (bytes.len() >= 10
                && header_magic(bytes@) && header_version(bytes@) == VERSION as int
                && header_count(bytes@) <= MAX_OPS as int
                && header_count(bytes@) > bytes.len() as int - 10),
            Err(_) => false,
        },
{""",
        "depends": ["header-constants", "scalar-read"], "evidence": "header-read-kernel",
        "scope": "complete 10-byte header validation and error classification, before allocation; not payload parsing or total heap budget",
        "mutations": [
            ("wrong-count", "Ok(op_count)", "Ok(0)"),
            ("wrong-version-error", "Err(BatchError::UnsupportedVersion(version))", "Err(BatchError::BadMagic)"),
        ],
    },
    "bounded-payload": {
        "source": "src/batch.rs", "marker": "BOUNDED PAYLOAD", "property": "UI-DECODE-PAYLOAD-001",
        "signature": "fn bounded_bytes(&mut self, max_len: u32) -> Result<&'a [u8], BatchError> {",
        "spec_prelude": """
pub open spec fn payload_length(s: Seq<u8>, pos: int) -> int
    recommends 0 <= pos, pos + 4 <= s.len(),
{
    s[pos] as int + 256 * s[pos + 1] as int
        + 65536 * s[pos + 2] as int + 16777216 * s[pos + 3] as int
}
""",
        "contract": """fn bounded_bytes(&mut self, max_len: u32) -> (result: Result<&'a [u8], BatchError>)
    ensures
        final(self).bytes@ == old(self).bytes@,
        match result {
            Ok(payload) => old(self).pos as int + 4 <= old(self).bytes.len()
                && payload_length(old(self).bytes@, old(self).pos as int) <= max_len as int
                && final(self).pos as int == old(self).pos as int + 4
                    + payload_length(old(self).bytes@, old(self).pos as int)
                && final(self).pos <= old(self).bytes.len()
                && payload.len() as int == payload_length(old(self).bytes@, old(self).pos as int)
                && payload@ == old(self).bytes@.subrange(old(self).pos as int + 4, final(self).pos as int),
            Err(BatchError::TooLarge) => old(self).pos as int + 4 <= old(self).bytes.len()
                && payload_length(old(self).bytes@, old(self).pos as int) > max_len as int
                && final(self).pos as int == old(self).pos as int + 4,
            Err(BatchError::Truncated) =>
                ((old(self).pos as int + 4 > old(self).bytes.len()
                    && final(self).pos == old(self).pos)
                || (old(self).pos as int + 4 <= old(self).bytes.len()
                    && payload_length(old(self).bytes@, old(self).pos as int) <= max_len as int
                    && old(self).pos as int + 4 + payload_length(old(self).bytes@, old(self).pos as int)
                        > old(self).bytes.len()
                    && final(self).pos as int == old(self).pos as int + 4)),
            Err(_) => false,
        },
{""",
        "depends": ["scalar-read"], "evidence": "bounded-payload-kernel",
        "scope": "length-prefixed borrowed payload, exact bytes, length limit and failure cursor states; not UTF-8 conversion or global heap budget",
        "mutations": [
            ("bypass-limit", "if len > max_len", "if false"),
            ("wrong-length", "self.take(len as usize)", "self.take(0)"),
        ],
    },
    "surface-row": {
        "source": "src/surface.rs", "marker": "SURFACE ROW", "property": "UI-SURFACE-ROW-001",
        "signature": "pub fn rgba8_row_bytes(width: u32, max_bytes: usize) -> Option<usize> {",
        "contract": """pub fn rgba8_row_bytes(width: u32, max_bytes: usize) -> (result: Option<usize>)
    ensures
        match result {
            Some(bytes) => width > 0 && bytes as int == width as int * 4 && bytes <= max_bytes,
            None => width == 0 || width as int * 4 > max_bytes as int,
        },
{""",
        "depends": [], "evidence": "surface-row-kernel",
        "scope": "packed RGBA8 row admission only; not stride, height, allocation or full surface validation",
        "mutations": [("wrong-row-size", "Some(width as usize * 4)", "Some(width as usize * 3)"),
                      ("reject-exact-budget", "> (max_bytes as u64) / 4", ">= (max_bytes as u64) / 4")],
    },
    "surface-span": {
        "source": "src/surface.rs", "marker": "SURFACE SPAN", "property": "UI-SURFACE-SPAN-001",
        "signature": "pub fn surface_span(\n    row_bytes: usize,\n    height: usize,\n    stride: usize,\n    capacity: usize,\n) -> Option<usize> {",
        "contract": """pub fn surface_span(row_bytes: usize, height: usize, stride: usize, capacity: usize) -> (result: Option<usize>)
    ensures
        match result {
            Some(end) => row_bytes > 0 && height > 0 && stride >= row_bytes && end <= capacity
                && end as int == (height as int - 1) * stride as int + row_bytes as int,
            None => row_bytes == 0 || height == 0 || stride < row_bytes
                || (height as int - 1) * stride as int + row_bytes as int > capacity as int,
        },
{""",
        "depends": [], "evidence": "surface-span-kernel",
        "scope": "numeric surface row/height/stride/capacity admission, exact last active byte; not allocation or permission/lifecycle",
        "mutations": [("wrong-surface-end", "        Some(end)\n    } else", "        Some(prefix)\n    } else"),
                      ("reject-exact-capacity", "if end <= capacity", "if end < capacity")],
    },
    "surface-borrow": {
        "source": "src/surface.rs", "marker": "SURFACE BORROW", "property": "UI-SURFACE-BORROW-001",
        "signature": "pub fn surface_row(\n    bytes: &[u8],\n    row_bytes: usize,\n    height: usize,\n    stride: usize,\n    row: usize,\n) -> Option<&[u8]> {",
        "contract": """pub fn surface_row(bytes: &[u8], row_bytes: usize, height: usize, stride: usize, row: usize) -> (result: Option<&[u8]>)
    ensures
        match result {
            Some(slice) => row < height && row_bytes > 0 && height > 0 && stride >= row_bytes
                && (height as int - 1) * stride as int + row_bytes as int <= bytes.len() as int
                && slice.len() == row_bytes
                && slice@ == bytes@.subrange(row as int * stride as int, row as int * stride as int + row_bytes as int),
            None => row >= height || row_bytes == 0 || height == 0 || stride < row_bytes
                || (height as int - 1) * stride as int + row_bytes as int > bytes.len() as int,
        },
{""",
        "depends": ["surface-span"], "evidence": "surface-borrow-kernel",
        "spec_prelude": "broadcast use vstd::arithmetic::mul::lemma_mul_inequality;",
        "scope": "read-only active row from an actual byte slice; whole declared span checked; excludes external resource/session ownership",
        "mutations": [("wrong-row-start", "row.checked_mul(stride)", "0usize.checked_mul(stride)"),
                      ("empty-row", "Some(&bytes[start..end])", "Some(&bytes[start..start])")],
    },
    "surface-write": {
        "source": "src/surface.rs", "marker": "SURFACE WRITE", "property": "UI-SURFACE-WRITE-001",
        "signature": "pub fn write_surface_row(\n    bytes: &mut [u8],\n    row_bytes: usize,\n    height: usize,\n    stride: usize,\n    row: usize,\n    source: &[u8],\n) -> bool {",
        "contract": """pub fn write_surface_row(bytes: &mut [u8], row_bytes: usize, height: usize, stride: usize, row: usize, source: &[u8]) -> (result: bool)
    ensures
        final(bytes).len() == old(bytes).len(),
        result == (source.len() == row_bytes && row < height && row_bytes > 0 && height > 0 && stride >= row_bytes
            && (height as int - 1) * stride as int + row_bytes as int <= old(bytes).len() as int),
        !result ==> final(bytes)@ == old(bytes)@,
        result ==> forall|i: int| 0 <= i < old(bytes).len() ==> (
            final(bytes)@[i] == if row as int * stride as int <= i < row as int * stride as int + row_bytes as int {
                source@[i - row as int * stride as int]
            } else {
                old(bytes)@[i]
            }),
{""",
        "depends": ["surface-borrow"], "evidence": "surface-write-kernel",
        "scope": "single synchronous active-row copy into a valid Rust mutable slice; exact source bytes and frame preservation, failure unchanged; not IPC permission or concurrent publication",
        "mutations": [("skip-write", "    bytes[start..end].copy_from_slice(source);", "    // deliberately omit the copy"),
                      ("wrong-write-start", "let start = row * stride;", "let start = 0usize;")],
    },
    "surface-view": {
        "source": "src/surface.rs", "marker": "SURFACE VIEW", "property": "UI-SURFACE-VIEW-001",
        "signature": "pub fn new(bytes: &'a [u8], width: u32, height: usize, stride: usize) -> Option<Self> {",
        "spec_prelude": """
pub closed spec fn surface_view_model(view: &Rgba8SurfaceView) -> (Seq<u8>, u32, usize, usize, usize) {
    (view.bytes@, view.width, view.height, view.stride, view.row_bytes)
}
""",
        "contract": """pub fn new(bytes: &'a [u8], width: u32, height: usize, stride: usize) -> (result: Option<Self>)
    ensures
        match result {
            Some(view) => width > 0 && height > 0 && stride as int >= width as int * 4
                && (height as int - 1) * stride as int + width as int * 4 <= bytes.len() as int
                && surface_view_model(&view).0 == bytes@ && surface_view_model(&view).1 == width && surface_view_model(&view).2 == height
                && surface_view_model(&view).3 == stride && surface_view_model(&view).4 as int == width as int * 4,
            None => width == 0 || height == 0 || (stride as int) < width as int * 4
                || (height as int - 1) * stride as int + width as int * 4 > bytes.len() as int,
        },
{
    reveal(surface_view_model);""",
        "extra_annotations": [
            ("pub fn row(&self, row: usize) -> Option<&'a [u8]> {", """pub fn row(&self, row: usize) -> (result: Option<&'a [u8]>)
    ensures
        match result {
            Some(slice) => row < surface_view_model(self).2 && surface_view_model(self).4 > 0 && surface_view_model(self).2 > 0 && surface_view_model(self).3 >= surface_view_model(self).4
                && (surface_view_model(self).2 as int - 1) * surface_view_model(self).3 as int + surface_view_model(self).4 as int <= surface_view_model(self).0.len() as int
                && slice.len() == surface_view_model(self).4
                && slice@ == surface_view_model(self).0.subrange(row as int * surface_view_model(self).3 as int, row as int * surface_view_model(self).3 as int + surface_view_model(self).4 as int),
            None => row >= surface_view_model(self).2 || surface_view_model(self).4 == 0 || surface_view_model(self).2 == 0 || surface_view_model(self).3 < surface_view_model(self).4
                || (surface_view_model(self).2 as int - 1) * surface_view_model(self).3 as int + surface_view_model(self).4 as int > surface_view_model(self).0.len() as int,
        },
{
    reveal(surface_view_model);"""),
            ("pub fn copy_row_to(&self, row: usize, destination: &mut [u8]) -> bool {", """pub fn copy_row_to(&self, row: usize, destination: &mut [u8]) -> (result: bool)
    ensures
        final(destination).len() == old(destination).len(),
        result == (row < surface_view_model(self).2 && surface_view_model(self).4 > 0 && surface_view_model(self).2 > 0
            && surface_view_model(self).3 >= surface_view_model(self).4
            && (surface_view_model(self).2 as int - 1) * surface_view_model(self).3 as int + surface_view_model(self).4 as int <= surface_view_model(self).0.len() as int
            && old(destination).len() == surface_view_model(self).4),
        result ==> final(destination)@ == surface_view_model(self).0.subrange(
            row as int * surface_view_model(self).3 as int, row as int * surface_view_model(self).3 as int + surface_view_model(self).4 as int),
        !result ==> final(destination)@ == old(destination)@,
{ reveal(surface_view_model);"""),
            ("pub fn width(&self) -> u32 {", "pub fn width(&self) -> (result: u32) ensures result == surface_view_model(self).1, { reveal(surface_view_model);"),
            ("pub fn height(&self) -> usize {", "pub fn height(&self) -> (result: usize) ensures result == surface_view_model(self).2, { reveal(surface_view_model);"),
            ("pub fn stride(&self) -> usize {", "pub fn stride(&self) -> (result: usize) ensures result == surface_view_model(self).3, { reveal(surface_view_model);"),
        ],
        "function_count": 6,
        "depends": ["surface-row", "surface-borrow"], "evidence": "surface-view-kernel",
        "scope": "immutable checked RGBA8 view constructor, metadata, active-row access and checked row transfer; does not import resource handles or prove external permissions",
        "mutations": [("wrong-view-width", "                width,", "                width: 0,"),
                      ("wrong-view-row", "self.stride, row)", "self.stride, 0)"),
                      ("skip-view-copy", "        destination.copy_from_slice(source);", "        // omit destination write")],
    },
    "pixel-pack": {
        "source": "src/surface.rs", "marker": "PIXEL PACK", "property": "UI-PIXEL-PACK-001",
        "signature": "pub fn rgb8_word(red: u8, green: u8, blue: u8) -> u32 {",
        "contract": """pub fn rgb8_word(red: u8, green: u8, blue: u8) -> (result: u32)
    ensures result as int == red as int * 65536 + green as int * 256 + blue as int,
        result <= 16777215,
{""",
        "depends": [], "evidence": "pixel-pack-kernel",
        "scope": "numeric RGB8 software word; exact channels and zero high byte; not alpha blending or native byte order",
        "mutations": [("wrong-red-lane", "    red as u32 * 65536", "    red as u32 * 256")],
    },
    "pixel-error-type": {
        "source": "src/desktop.rs", "marker": "PIXEL ERROR", "suffix": "TYPE",
        "signature": None, "depends": [], "internal_only": True,
    },
    "pixel-convert": {
        "source": "src/desktop.rs", "marker": "PIXEL CONVERT", "property": "UI-PIXEL-CONVERT-001",
        "signature": "pub fn rgba_to_u32(pixels: &[u8], out: &mut [u32]) -> Result<(), PixelError> {",
        "contract": """pub fn rgba_to_u32(pixels: &[u8], out: &mut [u32]) -> (result: Result<(), PixelError>)
    ensures
        final(out).len() == old(out).len(),
        match result {
            Ok(()) => pixels.len() as int == old(out).len() as int * 4
                && forall|i: int| 0 <= i < old(out).len() ==> final(out)@[i] as int ==
                    pixels@[i * 4] as int * 65536 + pixels@[i * 4 + 1] as int * 256 + pixels@[i * 4 + 2] as int,
            Err(PixelError::LengthMismatch) => pixels.len() as int != old(out).len() as int * 4
                && final(out)@ == old(out)@,
        },
{""",
        "extra_annotations": [("while index < out.len() {", """while index < out.len()
        invariant
            index <= out.len(),
            out.len() == old(out).len(),
            pixels.len() as int == out.len() as int * 4,
            forall|i: int| 0 <= i < index ==> out@[i] as int ==
                pixels@[i * 4] as int * 65536 + pixels@[i * 4 + 1] as int * 256 + pixels@[i * 4 + 2] as int,
        decreases out.len() - index,
        {""")],
        "depends": ["pixel-pack", "pixel-error-type"], "evidence": "pixel-convert-kernel",
        "scope": "production RGBA8-to-word loop, exact RGB (alpha ignored), all lengths, mismatch unchanged; no window/GPU correctness claim",
        "mutations": [("wrong-success", "    Ok(())\n}", "    Err(PixelError::LengthMismatch)\n}"),
                      ("reject-empty", "    let mut index = 0usize;", "    if pixels.len() == 0 && out.len() == 0 { return Err(PixelError::LengthMismatch); }\n    let mut index = 0usize;")],
    },
    "geometry-point-type": {"source": "src/geometry.rs", "marker": "GEOMETRY POINT", "suffix": "TYPE", "signature": None, "depends": [], "internal_only": True},
    "geometry-size-type": {"source": "src/geometry.rs", "marker": "GEOMETRY SIZE", "suffix": "TYPE", "signature": None, "depends": [], "internal_only": True},
    "geometry-rect-type": {"source": "src/geometry.rs", "marker": "GEOMETRY RECT", "suffix": "TYPE", "signature": None, "depends": ["geometry-point-type", "geometry-size-type"], "internal_only": True},
    "canvas-storage-type": {"source": "src/raster.rs", "marker": "CANVAS STORAGE", "suffix": "TYPE", "signature": None, "depends": ["geometry-rect-type"], "internal_only": True},
    "canvas-import": {
        "source": "src/raster.rs", "marker": "CANVAS IMPORT", "property": "UI-CANVAS-IMPORT-001",
        "signature": "pub fn from_rgba8(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, Vec<u8>> {",
        "spec_prelude": """pub closed spec fn canvas_model(canvas: &Canvas) -> (u32, u32, Seq<u8>, int) {
    (canvas.width, canvas.height, canvas.pixels@, canvas.clip_stack@.len() as int)
}""",
        "contract": """pub fn from_rgba8(width: u32, height: u32, pixels: Vec<u8>) -> (result: Result<Self, Vec<u8>>)
    ensures match result {
        Ok(canvas) => width > 0 && height > 0 && pixels.len() as int == width as int * height as int * 4
            && canvas_model(&canvas) == (width, height, pixels@, 0int),
        Err(returned) => returned@ == pixels@
            && (width == 0 || height == 0 || pixels.len() as int != width as int * height as int * 4),
    },
{ reveal(canvas_model);
    assert((height as int - 1) * (width as int * 4) + width as int * 4 == width as int * height as int * 4) by(nonlinear_arith);
    assert(width > 0 && height > 0 ==> width as int * height as int * 4 >= width as int * 4) by(nonlinear_arith);""",
        "extra_annotations": [
            ("pub fn width(&self) -> u32 {", "pub fn width(&self) -> (result: u32) ensures result == canvas_model(self).0, { reveal(canvas_model);"),
            ("pub fn height(&self) -> u32 {", "pub fn height(&self) -> (result: u32) ensures result == canvas_model(self).1, { reveal(canvas_model);"),
            ("pub fn pixels(&self) -> &[u8] {", "pub fn pixels(&self) -> (result: &[u8]) ensures result@ == canvas_model(self).2, { reveal(canvas_model);"),
        ],
        "function_count": 4,
        "depends": ["canvas-storage-type", "surface-row", "surface-span"], "evidence": "canvas-import-kernel",
        "scope": "owned packed RGBA8 Canvas import and existing metadata/pixel getters, exact buffer preservation and empty clipping; not allocating new/resize or raster operations",
        "mutations": [("wrong-import-width", "                width,", "                width: 0,"),
                      ("drop-error-bytes", "_ => Err(pixels),", "_ => Err(Vec::new()),")],
    },
    "color-type": {"source": "src/style.rs", "marker": "COLOR", "suffix": "TYPE", "signature": None, "depends": [], "internal_only": True},
    "canvas-pixel": {
        "source": "src/raster.rs", "marker": "CANVAS PIXEL READ", "property": "UI-CANVAS-PIXEL-001",
        "signature": "pub fn pixel(&self, x: u32, y: u32) -> Option<Color> {",
        "contract": """pub fn pixel(&self, x: u32, y: u32) -> (result: Option<Color>)
    requires canvas_model(self).2.len() == canvas_model(self).0 as int * canvas_model(self).1 as int * 4,
    ensures match result {
        None => x >= canvas_model(self).0 || y >= canvas_model(self).1,
        Some(color) => x < canvas_model(self).0 && y < canvas_model(self).1
            && color.r == canvas_model(self).2[(y as int * canvas_model(self).0 as int + x as int) * 4]
            && color.g == canvas_model(self).2[(y as int * canvas_model(self).0 as int + x as int) * 4 + 1]
            && color.b == canvas_model(self).2[(y as int * canvas_model(self).0 as int + x as int) * 4 + 2]
            && color.a == canvas_model(self).2[(y as int * canvas_model(self).0 as int + x as int) * 4 + 3],
    },
{ reveal(canvas_model);""",
        "extra_annotations": [
            ("        let index = ((y as usize * self.width as usize) + x as usize) * 4;",
             """        assert((y as int + 1) * (self.width as int) <= (self.height as int) * (self.width as int)) by(nonlinear_arith)
            requires (y as int) < (self.height as int), 0 <= (self.width as int);
        assert(y as int * self.width as int + self.width as int == (y as int + 1) * (self.width as int)) by(nonlinear_arith);
        assert(self.pixels@.len() == self.pixels.len() as int);
        assert(y as int * self.width as int + x as int + 1 <= self.width as int * self.height as int);
        assert((y as int * self.width as int + x as int) * 4 + 4 <= self.width as int * self.height as int * 4) by(nonlinear_arith)
            requires y as int * self.width as int + x as int + 1 <= self.width as int * self.height as int;
        assert(0 <= y as int * self.width as int) by(nonlinear_arith) requires 0 <= y as int, 0 <= self.width as int;
        let index = ((y as usize * self.width as usize) + x as usize) * 4;"""),
        ],
        "function_count": 1,
        "depends": ["canvas-import", "color-type"], "evidence": "canvas-pixel-kernel",
        "scope": "existing Canvas::pixel exact bounds rejection and RGBA channel read under the packed-length invariant that from_rgba8 establishes; not new/resize, blending or raster operations",
        "mutations": [("out-of-range-some", "            return None;\n        }\n        assert((y as int + 1)", "            return Some(Color { r: 0, g: 0, b: 0, a: 0 });\n        }\n        assert((y as int + 1)"),
                      ("swap-green-blue", "            g: self.pixels[index + 1],", "            g: self.pixels[index + 2],")],
    },
    "font-order": {
        "source": "src/native_adapter/native_fonts.rs", "marker": "FONT ORDER", "property": "UI-FONT-ORDER-001",
        "signature": "pub fn font_stack_count_admits(count: usize) -> bool {",
        "contract": """pub fn font_stack_count_admits(count: usize) -> (result: bool)
    ensures result == (1 <= count && count <= 16),
{""",
        "extra_annotations": [
            ("pub fn font_order_admits(seen: &[u64], id: u64) -> bool {",
             """pub fn font_order_admits(seen: &[u64], id: u64) -> (result: bool)
    ensures result == !seen@.contains(id),
{"""),
            ("    while index < seen.len() {",
             """    while index < seen.len()
        invariant index <= seen.len(), forall|j: int| 0 <= j < index ==> seen@[j] != id,
        decreases seen.len() - index,
    {"""),
            ("            return false;", "            assert(seen@[index as int] == id);\n            return false;"),
        ],
        "function_count": 2, "depends": [], "evidence": "font-order-kernel",
        "scope": "native injected font stack count admission and first-occurrence fallback-order predicate; not font parsing, upstream registration, family-name sorting or Parley fallback selection",
        "mutations": [("admit-empty-stack", "count >= 1 && count", "count >= 0 && count"),
                      ("admit-duplicate", "            return false;", "            return true;")],
    },
    "frame-reuse": {
        "source": "src/native_adapter/frame_schedule.rs", "marker": "FRAME REUSE", "property": "UI-FRAME-REUSE-001",
        "signature": """pub fn frame_reuse_admits(dirty: bool, cached_width: u32, cached_height: u32, cached_len: usize,
    width: u32, height: u32, expected: usize) -> bool {""",
        "contract": """pub fn frame_reuse_admits(dirty: bool, cached_width: u32, cached_height: u32, cached_len: usize,
    width: u32, height: u32, expected: usize) -> (result: bool)
    ensures result == (!dirty && cached_width == width && cached_height == height && cached_len == expected),
{""",
        "function_count": 1, "depends": [], "evidence": "frame-reuse-kernel",
        "scope": "on-demand frame reuse admission predicate only; not damage-marking completeness, cache storage, painting or presentation",
        "mutations": [("ignore-dirty", "\n    !dirty && cached_width", "\n    cached_width"),
                      ("ignore-extent", "cached_width == width && cached_height == height && cached_len == expected\n", "cached_len == expected\n")],
    },
    "damage-span": {
        "source": "src/native_adapter/frame_schedule.rs", "marker": "DAMAGE SPAN", "property": "UI-DAMAGE-SPAN-001",
        "signature": """pub fn damage_span_extend(span: Option<(u32, u32)>, at: u32) -> (u32, u32) {""",
        "contract": """pub fn damage_span_extend(span: Option<(u32, u32)>, at: u32) -> (result: (u32, u32))
    ensures
        result.0 <= at && at <= result.1,
        span matches None ==> result == (at, at),
        span matches Some((lo, hi)) ==> result.0 <= lo && hi <= result.1
            && (result.0 == lo || result.0 == at) && (result.1 == hi || result.1 == at),
{""",
        "function_count": 1, "depends": [], "evidence": "damage-span-kernel",
        "scope": "inclusive damage span extension (smallest span covering the old span and the new index) only; not the row/column scan of frame_damage, painting or presentation",
        "mutations": [("keep-low", "if at < lo { at } else { lo }", "lo"),
                      ("absent-span-from-zero", "None => (at, at)", "None => (0, at)")],
    },
    "frame-tick": {
        "source": "src/native_adapter/frame_schedule.rs", "marker": "FRAME TICK", "property": "UI-FRAME-TICK-001",
        "signature": """pub fn frame_tick_at(origin: u64, interval: u64, now: u64) -> Option<u64> {""",
        "contract": """pub fn frame_tick_at(origin: u64, interval: u64, now: u64) -> (result: Option<u64>)
    ensures
        (result matches None) == (interval == 0 || now < origin),
        result matches Some(t) ==> origin <= t && t <= now && now - t < interval
            && (t - origin) % (interval as int) == 0
            && t == now - (now - origin) % (interval as int),
{""",
        "extra_annotations": [
            ("    Some(now - (now - origin) % interval)",
             """    proof {
        let e = (now - origin) as int;
        let d = interval as int;
        vstd::arithmetic::div_mod::lemma_fundamental_div_mod(e, d);
        vstd::arithmetic::div_mod::lemma_mod_pos_bound(e, d);
        vstd::arithmetic::div_mod::lemma_mod_multiples_basic(e / d, d);
        vstd::arithmetic::mul::lemma_mul_is_commutative(d, e / d);
    }
    Some(now - (now - origin) % interval)"""),
        ],
        "function_count": 1, "depends": [], "evidence": "frame-tick-kernel",
        "scope": "refresh-period start (latest origin + k*interval not after now) and its None cases only; not the clock source, vsync or presentation timing",
        "mutations": [("unaligned-now", "    Some(now - (now - origin) % interval)", "    Some(now)"),
                      ("stale-origin", "    Some(now - (now - origin) % interval)", "    Some(origin)")],
    },
    "frame-pace-type": {"source": "src/native_adapter/frame_schedule.rs", "marker": "FRAME PACE", "suffix": "TYPE", "signature": None, "depends": [], "internal_only": True},
    "frame-pace": {
        "source": "src/native_adapter/frame_schedule.rs", "marker": "FRAME PACE", "property": "UI-FRAME-PACE-001",
        "signature": """pub fn frame_pace(dirty: bool, presented: Option<u64>, origin: u64, interval: u64, now: u64) -> Option<FramePace> {""",
        "contract": """pub fn frame_pace(dirty: bool, presented: Option<u64>, origin: u64, interval: u64, now: u64) -> (result: Option<FramePace>)
    ensures
        interval == 0 ==> result matches None,
        interval > 0 && !dirty ==> result == Some(FramePace::Idle),
        interval > 0 && dirty && now < origin ==> result == Some(FramePace::WaitUntil(origin)),
        interval > 0 && dirty && origin <= now ==> ({
            let tick = now - (now - origin) % (interval as int);
            let already = presented matches Some(last) && last >= tick;
            if !already {
                result == Some(FramePace::RenderNow(tick as u64))
            } else if tick + interval <= u64::MAX {
                result == Some(FramePace::WaitUntil((tick + interval) as u64))
            } else {
                result matches None
            }
        }),
{""",
        "function_count": 1, "depends": ["frame-tick", "frame-pace-type"], "evidence": "frame-pace-kernel",
        "scope": "per-period frame pacing decision (idle when clean, wait before first tick, one RenderNow per period, missed periods skipped, overflow rejected); not needs_frame completeness, the host clock, vsync or painting",
        "mutations": [("render-twice-per-period", "Some(last) => last >= tick,", "Some(last) => last > tick,"),
                      ("wake-when-clean", "        return Some(FramePace::Idle);", "        return Some(FramePace::RenderNow(now));")],
    },
    "text-offset": {
        "source": "src/native_adapter/text_geometry.rs", "marker": "TEXT OFFSET", "property": "UI-TEXT-OFFSET-001",
        "signature": """pub fn text_offset_admits(layout_len: usize, source_len: usize, offset: usize, on_boundary: bool) -> bool {""",
        "contract": """pub fn text_offset_admits(layout_len: usize, source_len: usize, offset: usize, on_boundary: bool) -> (result: bool)
    ensures result == (layout_len == source_len && offset <= source_len && on_boundary),
{""",
        "function_count": 1, "depends": [], "evidence": "text-offset-kernel",
        "scope": "layout-to-source text offset admission (identical length, in range with end included, character boundary) only; not the Parley caret/hit geometry, text equality or coordinate mapping",
        "mutations": [("ignore-length-mismatch", "\n    layout_len == source_len && offset", "\n    offset"),
                      ("exclude-text-end", "offset <= source_len && on_boundary\n}", "offset < source_len && on_boundary\n}")],
    },
    "text-range": {
        "source": "src/native_adapter/text_geometry.rs", "marker": "TEXT RANGE", "property": "UI-TEXT-RANGE-001",
        "signature": """pub fn text_range_admits(layout_len: usize, source_len: usize, start: usize, end: usize, start_on_boundary: bool, end_on_boundary: bool) -> bool {""",
        "contract": """pub fn text_range_admits(layout_len: usize, source_len: usize, start: usize, end: usize, start_on_boundary: bool, end_on_boundary: bool) -> (result: bool)
    ensures result == (layout_len == source_len && start <= end && end <= source_len && start_on_boundary && end_on_boundary),
{""",
        "function_count": 1, "depends": [], "evidence": "text-range-kernel",
        "scope": "layout-to-source text range admission (identical length, ordered, in range with end included, both ends on character boundaries) only; not the Parley selection geometry, text equality or coordinate mapping",
        "mutations": [("accept-reversed-range", "\n    layout_len == source_len && start <= end && end", "\n    layout_len == source_len && end"),
                      ("ignore-end-boundary", "start_on_boundary && end_on_boundary\n}", "start_on_boundary\n}")],
    },
    "gpu-readback": {
        "source": "src/native_adapter/gpu_render.rs", "marker": "GPU READBACK LAYOUT", "property": "UI-GPU-READBACK-001",
        "signature": """pub fn gpu_readback_layout(width: u32, height: u32) -> Option<(u32, u64)> {""",
        "contract": """pub fn gpu_readback_layout(width: u32, height: u32) -> (result: Option<(u32, u64)>)
    ensures
        (width == 0 || height == 0 || width > 65535 || height > 65535) ==> result matches None,
        (0 < width <= 65535 && 0 < height <= 65535) ==> (result matches Some((padded, bytes))
            && padded % 256 == 0 && padded >= width * 4 && padded < width * 4 + 256
            && bytes == padded * height),
{""",
        "extra_annotations": [("    Some((padded, padded as u64 * height as u64))",
            "    assert(padded as int * height as int <= 262144 * 65535) by (nonlinear_arith) requires 0 <= padded as int, padded as int <= 262144, 0 <= height as int, height as int <= 65535;\n    Some((padded, padded as u64 * height as u64))")],
        "function_count": 1, "depends": [], "evidence": "gpu-readback-kernel",
        "scope": "GPU readback buffer layout (extent admission shared with the CPU path, rows padded to the 256-byte copy alignment, buffer size) only; not the wgpu copy, mapping, device or Vello rendering",
        "mutations": [("drop-row-padding", "    let padded = (width * 4 + 255) / 256 * 256;", "    let padded = width * 4;"),
                      ("admit-oversize-height", "height > 65535 {", "height > 65536 {")],
    },
    "access-actions-type": {"source": "src/native_adapter/access_tree.rs", "marker": "ACCESS ACTIONS", "suffix": "TYPE", "signature": None, "depends": [], "internal_only": True},
    "access-actions": {
        "source": "src/native_adapter/access_tree.rs", "marker": "ACCESS ACTIONS", "property": "UI-ACCESS-ACTIONS-001",
        "signature": """pub fn access_actions(is_button: bool, focusable: bool, disabled: bool) -> AccessActions {""",
        "contract": """pub fn access_actions(is_button: bool, focusable: bool, disabled: bool) -> (result: AccessActions)
    ensures result.focus == (focusable && !disabled), result.activate == (is_button && !disabled),
{""",
        "function_count": 1, "depends": ["access-actions-type"], "evidence": "access-actions-kernel",
        "scope": "accessibility actions of a node already in the visible tree (focus iff enabled and focusable, activate iff enabled button) only; not visibility pruning, traversal, geometry or the focus/activate entry points themselves",
        "mutations": [("focus-when-disabled", "AccessActions { focus: focusable && !disabled,", "AccessActions { focus: focusable,"),
                      ("activate-any-role", "activate: is_button && !disabled }", "activate: !disabled }")],
    },
    "damage-rect": {
        "source": "src/native_adapter/frame_schedule.rs", "marker": "DAMAGE RECT", "property": "UI-DAMAGE-RECT-001",
        "signature": """pub fn damage_rect_admits(x: u32, y: u32, rect_width: u32, rect_height: u32, width: u32, height: u32) -> bool {""",
        "contract": """pub fn damage_rect_admits(x: u32, y: u32, rect_width: u32, rect_height: u32, width: u32, height: u32) -> (result: bool)
    ensures result == (rect_width > 0 && rect_height > 0
        && x as int + rect_width as int <= width as int && y as int + rect_height as int <= height as int),
{""",
        "function_count": 1, "depends": [], "evidence": "damage-rect-kernel",
        "scope": "damage rectangle admission (nonempty, fully inside the frame, no overflow) only; not the damage scan, the surface write loop or presentation",
        "mutations": [("admit-empty-rect", "    rect_width > 0 && rect_height > 0 && x <= width", "    x <= width"),
                      ("ignore-vertical-bound", "\n        && y <= height && rect_height <= height - y", "")],
    },
    "surface-segment": {
        "source": "src/surface.rs", "marker": "SURFACE SEGMENT WRITE", "property": "UI-SURFACE-SEGMENT-001",
        "signature": "pub fn write_surface_segment(\n    bytes: &mut [u8],\n    row_bytes: usize,\n    height: usize,\n    stride: usize,\n    row: usize,\n    offset: usize,\n    source: &[u8],\n) -> bool {",
        "contract": """pub fn write_surface_segment(bytes: &mut [u8], row_bytes: usize, height: usize, stride: usize, row: usize, offset: usize, source: &[u8]) -> (result: bool)
    ensures
        final(bytes).len() == old(bytes).len(),
        result == (offset as int + source.len() as int <= row_bytes as int && row < height && row_bytes > 0 && height > 0 && stride >= row_bytes
            && (height as int - 1) * stride as int + row_bytes as int <= old(bytes).len() as int),
        !result ==> final(bytes)@ == old(bytes)@,
        result ==> forall|i: int| 0 <= i < old(bytes).len() ==> (
            final(bytes)@[i] == if row as int * stride as int + offset as int <= i < row as int * stride as int + offset as int + source.len() as int {
                source@[i - row as int * stride as int - offset as int]
            } else {
                old(bytes)@[i]
            }),
{""",
        "depends": ["surface-borrow"], "evidence": "surface-segment-kernel",
        "scope": "single synchronous in-row segment copy into a valid Rust mutable slice; exact source bytes, all other bytes (other rows, rest of row, padding) preserved, failure unchanged; not IPC permission or concurrent publication",
        "mutations": [("skip-segment-write", "    bytes[start..end].copy_from_slice(source);", "    // deliberately omit the copy"),
                      ("ignore-segment-offset", "let start = row * stride + offset;", "let start = row * stride;")],
    },
    "role-type": {"source": "src/semantics.rs", "marker": "ROLE", "suffix": "TYPE", "signature": None, "depends": [], "internal_only": True},
    "role-activation": {
        "source": "src/keyboard.rs", "marker": "ROLE ACTIVATION", "property": "UI-ROLE-ACTIVATION-001",
        "signature": "pub fn is_activatable(self) -> bool {",
        "contract": """pub fn is_activatable(self) -> (result: bool)
    ensures result == (self == Role::Button || self == Role::Checkbox || self == Role::Radio || self == Role::Switch || self == Role::Link || self == Role::MenuItem),
{""",
        "extra_annotations": [("pub fn activates_with_space(self) -> bool {", """pub fn activates_with_space(self) -> (result: bool)
    ensures result == (self == Role::Button || self == Role::Checkbox || self == Role::Radio || self == Role::Switch || self == Role::MenuItem),
{""")],
        "function_count": 2, "depends": ["role-type"], "evidence": "role-activation-kernel",
        "scope": "two existing role activation classifiers; not keyboard routing, focus or assistive host integration",
        "mutations": [("remove-link", "                | Role::Link\n", ""), ("remove-space-menu", "Role::Button | Role::Checkbox | Role::Radio | Role::Switch | Role::MenuItem", "Role::Button | Role::Checkbox | Role::Radio | Role::Switch")],
    },
    "resource-gate": {
        "source": "src/resource.rs", "marker": "RESOURCE GATE", "property": "UI-RESOURCE-GATE-001",
        "signature": "pub fn new(session: u64, slot: u64) -> Self {",
        "spec_prelude": """pub closed spec fn resource_model(gate: &ResourceGate) -> (u64, u64, u64, bool) {
    (gate.session, gate.slot, gate.generation, gate.live)
}
pub open spec fn resource_allowed(state: (u64,u64,u64,bool), caller: u64, key: ResourceKey) -> bool {
    state.3 && state.2 != 0 && caller == state.0 && key.session == state.0 && key.slot == state.1 && key.generation == state.2
}""",
        "contract": """pub fn new(session: u64, slot: u64) -> (result: Self)
    ensures resource_model(&result) == (session, slot, 0u64, false),
{ reveal(resource_model);""",
        "extra_annotations": [
            ("pub fn issue(&mut self, caller_session: u64) -> Option<ResourceKey> {", """pub fn issue(&mut self, caller_session: u64) -> (result: Option<ResourceKey>)
    ensures match result {
        Some(key) => caller_session == resource_model(old(self)).0 && !resource_model(old(self)).3
            && resource_model(old(self)).2 < u64::MAX
            && resource_model(final(self)).0 == resource_model(old(self)).0
            && resource_model(final(self)).1 == resource_model(old(self)).1
            && resource_model(final(self)).2 as int == resource_model(old(self)).2 as int + 1
            && resource_model(final(self)).3 && resource_model(final(self)).2 > 0
            && key.session == resource_model(final(self)).0 && key.slot == resource_model(final(self)).1
            && key.generation == resource_model(final(self)).2,
        None => resource_model(final(self)) == resource_model(old(self))
            && (caller_session != resource_model(old(self)).0 || resource_model(old(self)).3 || resource_model(old(self)).2 == u64::MAX),
    },
{ reveal(resource_model);"""),
            ("pub fn allows(&self, caller_session: u64, key: ResourceKey) -> bool {", """pub fn allows(&self, caller_session: u64, key: ResourceKey) -> (result: bool)
    ensures result == resource_allowed(resource_model(self), caller_session, key),
{ reveal(resource_model);"""),
            ("pub fn revoke(&mut self, caller_session: u64, key: ResourceKey) -> bool {", """pub fn revoke(&mut self, caller_session: u64, key: ResourceKey) -> (result: bool)
    ensures result == resource_allowed(resource_model(old(self)), caller_session, key),
        resource_model(final(self)).0 == resource_model(old(self)).0,
        resource_model(final(self)).1 == resource_model(old(self)).1,
        resource_model(final(self)).2 == resource_model(old(self)).2,
        resource_model(final(self)).3 == (resource_model(old(self)).3 && !result),
{ reveal(resource_model);"""),
            ("pub fn retire(&mut self) {", """pub fn retire(&mut self)
    ensures resource_model(final(self)) == (resource_model(old(self)).0, resource_model(old(self)).1, u64::MAX, false),
{ reveal(resource_model);"""),
        ],
        "function_count": 5, "depends": [], "evidence": "resource-gate-kernel",
        "scope": "one synchronous host-owned UI slot, exact session/slot/generation admission, revocation and permanent host retirement, no wrap; trusted authenticated caller and unique lifetime of gate; not storage freeing, in-flight cancellation or IPC authentication",
        "mutations": [("duplicate-live-issue", "|| self.live ||", "|| false ||"), ("wrong-generation", "key.generation == self.generation", "key.generation != self.generation"),
                      ("reset-retired-generation", "self.generation = u64::MAX;", "self.generation = 0;")],
    },
    "modifiers-type": {"source": "src/input.rs", "marker": "MODIFIERS", "suffix": "TYPE", "signature": None, "depends": [], "internal_only": True},
    "key-phase-type": {"source": "src/input.rs", "marker": "KEY PHASE", "suffix": "TYPE", "signature": None, "depends": [], "internal_only": True},
    "key-event-type": {"source": "src/input.rs", "marker": "KEY EVENT", "suffix": "TYPE", "signature": None, "depends": ["modifiers-type", "key-phase-type"], "internal_only": True},
    "command-modifier": {
        "source": "src/keyboard.rs", "marker": "COMMAND MODIFIER", "property": "UI-KEY-MODIFIER-001",
        "signature": "fn no_command_modifier(event: &KeyEvent) -> bool {",
        "contract": """fn no_command_modifier(event: &KeyEvent) -> (result: bool)
    ensures result == (!event.modifiers.control && !event.modifiers.alt && !event.modifiers.meta),
{""",
        "depends": ["key-event-type"], "evidence": "command-modifier-kernel",
        "scope": "existing command-modifier predicate; shift/key/phase/repeat do not affect it; not full keyboard routing",
        "mutations": [("drop-meta-guard", "    !event.modifiers.control && !event.modifiers.alt && !event.modifiers.meta\n", "    !event.modifiers.control && !event.modifiers.alt\n")],
    },
    "bytes-slot": {
        "source": "src/resource.rs", "marker": "BYTES SLOT", "property": "UI-RESOURCE-BYTES-001",
        "signature": "pub fn new(session: u64, slot: u64) -> Self {",
        "spec_prelude": """pub closed spec fn bytes_slot_model(slot: &UiBytesSlot) -> ((u64,u64,u64,bool), Option<Seq<u8>>) {
    (resource_model(&slot.gate), match &slot.payload { Some(bytes) => Some(bytes@), None => None })
}
pub open spec fn bytes_slot_valid(state: ((u64,u64,u64,bool), Option<Seq<u8>>)) -> bool {
    state.0.3 == state.1.is_some() && (state.0.3 ==> state.0.2 > 0)
}""",
        "contract": """pub fn new(session: u64, slot: u64) -> (result: Self)
    ensures bytes_slot_model(&result) == ((session, slot, 0u64, false), None::<Seq<u8>>),
        bytes_slot_valid(bytes_slot_model(&result)),
{ reveal(bytes_slot_model); reveal(resource_model);""",
        "extra_annotations": [
            ("pub fn install(&mut self, caller_session: u64, bytes: Vec<u8>) -> Result<ResourceKey, Vec<u8>> {", """pub fn install(&mut self, caller_session: u64, bytes: Vec<u8>) -> (result: Result<ResourceKey, Vec<u8>>)
    requires bytes_slot_valid(bytes_slot_model(old(self))),
    ensures bytes_slot_valid(bytes_slot_model(final(self))),
        match result {
            Ok(key) => caller_session == bytes_slot_model(old(self)).0.0 && !bytes_slot_model(old(self)).0.3
                && bytes_slot_model(old(self)).0.2 < u64::MAX
                && bytes_slot_model(final(self)).0.0 == bytes_slot_model(old(self)).0.0
                && bytes_slot_model(final(self)).0.1 == bytes_slot_model(old(self)).0.1
                && bytes_slot_model(final(self)).0.2 as int == bytes_slot_model(old(self)).0.2 as int + 1
                && resource_allowed(bytes_slot_model(final(self)).0, caller_session, key)
                && bytes_slot_model(final(self)).1 == Some(bytes@),
            Err(returned) => returned@ == bytes@ && bytes_slot_model(final(self)) == bytes_slot_model(old(self))
                && (caller_session != bytes_slot_model(old(self)).0.0 || bytes_slot_model(old(self)).0.3 || bytes_slot_model(old(self)).0.2 == u64::MAX),
        },
{ reveal(bytes_slot_model); reveal(resource_model);"""),
            ("pub fn bytes(&self, caller_session: u64, key: ResourceKey) -> Option<&[u8]> {", """pub fn bytes(&self, caller_session: u64, key: ResourceKey) -> (result: Option<&[u8]>)
    requires bytes_slot_valid(bytes_slot_model(self)),
    ensures match result {
        Some(bytes) => resource_allowed(bytes_slot_model(self).0, caller_session, key)
            && bytes_slot_model(self).1 == Some(bytes@),
        None => !resource_allowed(bytes_slot_model(self).0, caller_session, key),
    },
{ reveal(bytes_slot_model); reveal(resource_model);"""),
            ("pub fn release(&mut self, caller_session: u64, key: ResourceKey) -> Option<Vec<u8>> {", """pub fn release(&mut self, caller_session: u64, key: ResourceKey) -> (result: Option<Vec<u8>>)
    requires bytes_slot_valid(bytes_slot_model(old(self))),
    ensures bytes_slot_valid(bytes_slot_model(final(self))),
        bytes_slot_model(final(self)).0.0 == bytes_slot_model(old(self)).0.0,
        bytes_slot_model(final(self)).0.1 == bytes_slot_model(old(self)).0.1,
        bytes_slot_model(final(self)).0.2 == bytes_slot_model(old(self)).0.2,
        match result {
            Some(bytes) => resource_allowed(bytes_slot_model(old(self)).0, caller_session, key)
                && bytes_slot_model(old(self)).1 == Some(bytes@)
                && bytes_slot_model(final(self)).1 == None::<Seq<u8>> && !bytes_slot_model(final(self)).0.3,
            None => !resource_allowed(bytes_slot_model(old(self)).0, caller_session, key)
                && bytes_slot_model(final(self)) == bytes_slot_model(old(self)),
        },
{ reveal(bytes_slot_model); reveal(resource_model);"""),
            ("pub fn retire(&mut self) -> Option<Vec<u8>> {", """pub fn retire(&mut self) -> (result: Option<Vec<u8>>)
    ensures bytes_slot_valid(bytes_slot_model(final(self))),
        bytes_slot_model(final(self)).0 == (bytes_slot_model(old(self)).0.0, bytes_slot_model(old(self)).0.1, u64::MAX, false),
        bytes_slot_model(final(self)).1 == None::<Seq<u8>>,
        match result {
            Some(bytes) => bytes_slot_model(old(self)).1 == Some(bytes@),
            None => bytes_slot_model(old(self)).1 == None::<Seq<u8>>,
        },
{ reveal(bytes_slot_model); reveal(resource_model);"""),
        ],
        "function_count": 5, "depends": ["resource-gate"], "evidence": "bytes-slot-kernel",
        "scope": "owned Vec installation, authenticated identity lookup, exact borrowed bytes and ownership-returning release and permanent host retirement; private storage invariant established by new and preserved by all methods; host identity/unique gate boundary remains; not font parsing, IPC or in-flight external cancellation",
        "mutations": [("install-empty-payload", "self.payload = Some(bytes);", "self.payload = Some(Vec::new());"),
                      ("wrong-caller-read", "if !self.gate.allows(caller_session, key)", "if !self.gate.allows(caller_session ^ 1, key)"),
                      ("skip-slot-retirement", "        self.gate.retire();", "        // omit permanent retirement")],
    },
    "text-class": {'source': 'src/text.rs', 'marker': 'TEXT CLASS', 'property': 'UI-TEXT-CLASS-001', 'signature': 'fn is_wide(ch: char) -> bool {', 'contract': 'fn is_wide(ch: char) -> (result: bool)\n    ensures result == (((ch as u32) >= 4352 && (ch as u32) <= 4447) || ((ch as u32) >= 11904 && (ch as u32) <= 42191) || ((ch as u32) >= 44032 && (ch as u32) <= 55203) || ((ch as u32) >= 63744 && (ch as u32) <= 64255) || ((ch as u32) >= 65072 && (ch as u32) <= 65103) || ((ch as u32) >= 65280 && (ch as u32) <= 65376) || ((ch as u32) >= 65504 && (ch as u32) <= 65510) || ((ch as u32) >= 127744 && (ch as u32) <= 129791)),\n{', 'extra_annotations': [('fn is_zero_width(ch: char) -> bool {', 'fn is_zero_width(ch: char) -> (result: bool)\n    ensures result == (((ch as u32) >= 768 && (ch as u32) <= 879) || ((ch as u32) >= 6832 && (ch as u32) <= 6911) || ((ch as u32) >= 7616 && (ch as u32) <= 7679) || ((ch as u32) >= 8203 && (ch as u32) <= 8205) || ((ch as u32) >= 8400 && (ch as u32) <= 8447) || ((ch as u32) >= 65024 && (ch as u32) <= 65039) || ((ch as u32) >= 65056 && (ch as u32) <= 65071) || ((ch as u32) >= 917760 && (ch as u32) <= 917999)),\n{')], 'function_count': 2, 'depends': [], 'evidence': 'text-class-kernel', 'scope': 'two existing approximate-metrics range classifiers over all Rust chars; exact listed ranges only, not Unicode-complete width/grapheme/bidi or shaping correctness', 'mutations': [('drop-wide-range', '        | 0x1F300..=0x1FAFF\n', ''), ('drop-variation-range', '        | 0xE0100..=0xE01EF\n', '')]},
    "node-id-type": {"source": "src/scene.rs", "marker": "NODE ID", "suffix": "TYPE", "signature": None, "depends": [], "internal_only": True},
    "focus-storage-type": {"source": "src/semantics.rs", "marker": "FOCUS STORAGE", "suffix": "TYPE", "signature": None, "depends": ["node-id-type"], "internal_only": True},
    "focus-cleanup": {
        "source": "src/semantics.rs", "marker": "FOCUS CLEANUP", "property": "UI-FOCUS-CLEANUP-001",
        "signature": "pub fn blur(&mut self) {",
        "spec_prelude": """pub closed spec fn focus_model(manager: &FocusManager) -> (Seq<NodeId>, Option<NodeId>) {
    (manager.order@, manager.focused)
}""",
        "contract": """pub fn blur(&mut self)
    ensures focus_model(final(self)).0 == focus_model(old(self)).0,
        focus_model(final(self)).1 == None::<NodeId>,
{ reveal(focus_model);""",
        "extra_annotations": [
            ("pub fn order(&self) -> &[NodeId] {", "pub fn order(&self) -> (result: &[NodeId]) ensures result@ == focus_model(self).0, { reveal(focus_model);"),
            ("pub fn focused(&self) -> Option<NodeId> {", "pub fn focused(&self) -> (result: Option<NodeId>) ensures result == focus_model(self).1, { reveal(focus_model);"),
        ],
        "function_count": 3, "depends": ["focus-storage-type"], "evidence": "focus-cleanup-kernel",
        "scope": "existing blur clears focus and preserves exact traversal order; two metadata getters; not focus traversal, routing or host cancellation delivery",
        "mutations": [("wrong-clear-focus", "        self.focused = None;", "        self.focused = Some(NodeId(0));"),
                      ("wrong-order-view", "        &self.order\n", "        &self.order[0..0]\n")],
    },
    "keyboard-storage-type": {"source": "src/keyboard.rs", "marker": "KEYBOARD STORAGE", "suffix": "TYPE", "signature": None, "depends": ["focus-storage-type"], "internal_only": True},
    "keyboard-focus": {
        "source": "src/keyboard.rs", "marker": "KEYBOARD FOCUS", "property": "UI-KEYBOARD-FOCUS-001",
        "signature": "pub fn take_focus(&mut self) -> Option<NodeId> {",
        "spec_prelude": """pub closed spec fn keyboard_focus_model(router: &KeyboardRouter) -> (Seq<NodeId>, Option<NodeId>) {
    focus_model(&router.focus)
}""",
        "contract": """pub fn take_focus(&mut self) -> (result: Option<NodeId>)
    ensures result == keyboard_focus_model(old(self)).1,
        keyboard_focus_model(final(self)).1 == None::<NodeId>,
        keyboard_focus_model(final(self)).0 == keyboard_focus_model(old(self)).0,
{ reveal(keyboard_focus_model);""",
        "extra_annotations": [
            ("pub fn focused(&self) -> Option<NodeId> {", "pub fn focused(&self) -> (result: Option<NodeId>) ensures result == keyboard_focus_model(self).1, { reveal(keyboard_focus_model);"),
            ("pub fn blur(&mut self) {", "pub fn blur(&mut self) ensures keyboard_focus_model(final(self)).1 == None::<NodeId>, keyboard_focus_model(final(self)).0 == keyboard_focus_model(old(self)).0, { reveal(keyboard_focus_model);")
        ],
        "function_count": 3, "depends": ["keyboard-storage-type", "focus-cleanup"], "evidence": "keyboard-focus-kernel",
        "scope": "actual KeyboardRouter focused/blur delegates and take_focus exact previous target with focus cleared and traversal order preserved; not IME owner, pointer cancellation, session closure or delivery",
        "mutations": [("lost-previous-target", "        previous\n", "        None\n"),
                      ("skip-keyboard-clear", "        let previous = self.focus.focused();\n        self.focus.blur();", "        let previous = self.focus.focused();")],
    },
    "focus-error-type": {"source": "src/semantics.rs", "marker": "FOCUS ERROR", "suffix": "TYPE", "signature": None, "depends": ["node-id-type"], "internal_only": True},
    "focus-select": {
        "source": "src/semantics.rs", "marker": "FOCUS SELECT", "property": "UI-FOCUS-SELECT-001",
        "signature": "pub fn focus(&mut self, node_id: NodeId) -> Result<(), FocusError> {",
        "contract": """pub fn focus(&mut self, node_id: NodeId) -> (result: Result<(), FocusError>)
    ensures focus_model(final(self)).0 == focus_model(old(self)).0,
        match result {
            Ok(()) => focus_model(old(self)).0.contains(node_id) && focus_model(final(self)).1 == Some(node_id),
            Err(FocusError::NotFocusable(id)) => id == node_id && !focus_model(old(self)).0.contains(node_id)
                && focus_model(final(self)) == focus_model(old(self)),
        },
{ reveal(focus_model);""",
        "extra_annotations": [("        while index < self.order.len() {", """        while index < self.order.len()
            invariant index <= self.order.len(),
                self.order@ == focus_model(old(self)).0,
                self.focused == focus_model(old(self)).1,
                forall|j: int| 0 <= j < index ==> self.order@[j] != node_id,
            decreases self.order.len() - index,
        {""")],
        "function_count": 1, "depends": ["focus-error-type", "focus-cleanup"], "evidence": "focus-select-kernel",
        "scope": "existing focus selection accepts exactly traversal-order members; rejection returns requested node and preserves all state; not tree construction or traversal navigation",
        "mutations": [("wrong-selected-node", "                self.focused = Some(node_id);", "                self.focused = Some(NodeId(0));"),
                      ("wrong-error-node", "        Err(FocusError::NotFocusable(node_id))", "        Err(FocusError::NotFocusable(NodeId(0)))")],
    },
    "input-retire": {
        "source": "src/keyboard.rs", "marker": "INPUT RETIRE", "property": "UI-INPUT-RETIRE-001",
        "signature": "pub fn retire_focus(&mut self, gate: &mut ResourceGate) -> Option<NodeId> {",
        "contract": """pub fn retire_focus(&mut self, gate: &mut ResourceGate) -> (result: Option<NodeId>)
    ensures result == keyboard_focus_model(old(self)).1,
        keyboard_focus_model(final(self)).1 == None::<NodeId>,
        keyboard_focus_model(final(self)).0 == keyboard_focus_model(old(self)).0,
        resource_model(final(gate)) == (resource_model(old(gate)).0, resource_model(old(gate)).1, u64::MAX, false),
{ """,
        "function_count": 1, "depends": ["resource-gate", "keyboard-focus"], "evidence": "input-retire-kernel",
        "scope": "actual host-only gate retirement plus focus extraction; host must pair ownership and check every route; not IME/pointer cancellation, external delivery, global sessions or IPC concurrency",
        "mutations": [("omit-input-retirement", "        gate.retire();", "        // omit admission shutdown"),
                      ("discard-input-target", "        self.take_focus()\n", "        self.take_focus(); None\n")],
    },
    "hover-change-type": {"source": "src/pointer.rs", "marker": "HOVER CHANGE", "suffix": "TYPE", "signature": None, "depends": ["node-id-type"], "internal_only": True},
    "hover-storage-type": {"source": "src/pointer.rs", "marker": "HOVER STORAGE", "suffix": "TYPE", "signature": None, "depends": ["hover-change-type"], "internal_only": True},
    "hover-update": {
        "source": "src/pointer.rs", "marker": "HOVER UPDATE", "property": "UI-HOVER-UPDATE-001",
        "signature": "pub fn update(&mut self, target: Option<NodeId>) -> Vec<HoverChange> {",
        "spec_prelude": """pub closed spec fn hover_model(tracker: &HoverTracker) -> Option<NodeId> { tracker.current }
pub open spec fn hover_changes(old: Option<NodeId>, target: Option<NodeId>) -> Seq<HoverChange> {
    if old == target { Seq::empty() } else {
        (match old { Some(id) => seq![HoverChange::Leave(id)], None => Seq::empty() })
        + (match target { Some(id) => seq![HoverChange::Enter(id)], None => Seq::empty() })
    }
}""",
        "contract": """pub fn update(&mut self, target: Option<NodeId>) -> (result: Vec<HoverChange>)
    ensures hover_model(final(self)) == target, result@ == hover_changes(hover_model(old(self)), target),
{ reveal(hover_model);""",
        "extra_annotations": [
            ("pub fn hovered(&self) -> Option<NodeId> {", "pub fn hovered(&self) -> (result: Option<NodeId>) ensures result == hover_model(self), { reveal(hover_model);"),
            ("pub fn clear(&mut self) -> Vec<HoverChange> {", "pub fn clear(&mut self) -> (result: Vec<HoverChange>) ensures hover_model(final(self)) == None::<NodeId>, result@ == hover_changes(hover_model(old(self)), None::<NodeId>), {")
        ],
        "function_count": 3, "depends": ["hover-storage-type"], "evidence": "hover-update-kernel",
        "scope": "actual hover getter/update/clear: exact Leave-before-Enter, no events on unchanged target, final ownership; not hit testing, capture cancellation or external delivery",
        "mutations": [("wrong-leave-kind", "            out.push(HoverChange::Leave(old));", "            out.push(HoverChange::Enter(old));"),
                      ("retain-old-hover", "        self.current = target;", "        // omit target update")],
    },
    "hover-retire": {
        "source": "src/pointer.rs", "marker": "HOVER RETIRE", "property": "UI-HOVER-RETIRE-001",
        "signature": "pub fn retire_hover(&mut self, gate: &mut ResourceGate) -> Vec<HoverChange> {",
        "contract": """pub fn retire_hover(&mut self, gate: &mut ResourceGate) -> (result: Vec<HoverChange>)
    ensures hover_model(final(self)) == None::<NodeId>,
        result@ == hover_changes(hover_model(old(self)), None::<NodeId>),
        resource_model(final(gate)) == (resource_model(old(gate)).0, resource_model(old(gate)).1, u64::MAX, false),
{ """,
        "function_count": 1, "depends": ["resource-gate", "hover-update"], "evidence": "hover-retire-kernel",
        "scope": "real hover leave intent plus permanent gate retirement; host ownership pairing and guarded entry required; no capture/press/gesture/IME cancellation or delivery proof",
        "mutations": [("omit-hover-retirement", "        gate.retire();", "        // omit admission shutdown"),
                      ("discard-hover-leave", "        self.clear()\n", "        self.clear(); Vec::new()\n")],
    },
    'pointer-id-type': {'source': 'src/input.rs', 'marker': 'POINTER ID', 'suffix': 'TYPE', 'signature': None, 'depends': [], 'internal_only': True},
    'pointer-order': {'source': 'src/input.rs', 'marker': 'POINTER ORDER', 'property': 'UI-POINTER-ORDER-001', 'signature': 'fn eq(&self, other: &Self) -> bool {', 'contract': 'fn eq(&self, other: &Self) -> (result: bool) ensures result == (self.0 == other.0), {', 'spec_prelude': 'use vstd::std_specs::cmp::{PartialEqSpecImpl, PartialOrdSpecImpl, OrdSpecImpl};\npub open spec fn pointer_cmp(a: u64, b: u64) -> Ordering {\n    if a < b { Ordering::Less } else if a > b { Ordering::Greater } else { Ordering::Equal }\n}\nimpl PartialEqSpecImpl for PointerId {\n    open spec fn obeys_eq_spec() -> bool { true }\n    open spec fn eq_spec(&self, other: &Self) -> bool { self.0 == other.0 }\n}\nimpl PartialOrdSpecImpl for PointerId {\n    open spec fn obeys_partial_cmp_spec() -> bool { true }\n    open spec fn partial_cmp_spec(&self, other: &Self) -> Option<Ordering> { Some(pointer_cmp(self.0, other.0)) }\n}\nimpl OrdSpecImpl for PointerId {\n    open spec fn obeys_cmp_spec() -> bool { true }\n    open spec fn cmp_spec(&self, other: &Self) -> Ordering {\n        pointer_cmp(self.0, other.0)\n    }\n}\nproof fn pointer_order_laws()\n    ensures vstd::laws_cmp::obeys_cmp::<PointerId>(),\n{\n    reveal(vstd::laws_eq::obeys_eq_spec_properties);\n    reveal(vstd::laws_cmp::obeys_cmp_partial_ord);\n    reveal(vstd::laws_cmp::obeys_cmp_ord);\n    reveal(vstd::laws_cmp::obeys_partial_cmp_spec_properties);\n}\n', 'function_count': 3, 'depends': ['pointer-id-type'], 'evidence': 'pointer-order-kernel', 'scope': 'actual PointerId equality/partial-order/total-order bodies and laws; same original u64 identity, no pointer routing or session authorization claim', 'extra_annotations': [('fn partial_cmp(&self, other: &Self) -> Option<Ordering> {', 'fn partial_cmp(&self, other: &Self) -> (result: Option<Ordering>) ensures result == Some(pointer_cmp(self.0, other.0)), {'), ('fn cmp(&self, other: &Self) -> Ordering {', 'fn cmp(&self, other: &Self) -> (result: Ordering) ensures result == pointer_cmp(self.0, other.0), {')], 'mutations': [('wrong-pointer-equality', '        self.0 == other.0\n', '        false\n'), ('wrong-pointer-order', '            Ordering::Less\n', '            Ordering::Greater\n')]},
    'pointer-kind-types': {'source': 'src/input.rs', 'marker': 'POINTER KIND', 'suffix': 'TYPES', 'signature': None, 'depends': [], 'internal_only': True},
    'pointer-event-type': {'source': 'src/input.rs', 'marker': 'POINTER EVENT', 'suffix': 'TYPE', 'signature': None, 'depends': ['pointer-id-type', 'pointer-kind-types', 'geometry-point-type', 'geometry-size-type', 'modifiers-type'], 'internal_only': True},
    'pointer-tracker-type': {'source': 'src/input.rs', 'marker': 'POINTER TRACKER', 'suffix': 'TYPE', 'signature': None, 'depends': ['pointer-event-type'], 'internal_only': True},
    'pointer-router-type': {'source': 'src/pointer.rs', 'marker': 'POINTER ROUTER', 'suffix': 'TYPE', 'signature': None, 'depends': ['pointer-tracker-type', 'node-id-type'], 'internal_only': True},
    'capture-query': {'source': 'src/pointer.rs', 'marker': 'CAPTURE QUERY', 'property': 'UI-CAPTURE-QUERY-001', 'signature': 'pub fn captured_target(&self, pointer_id: PointerId) -> Option<NodeId> {', 'contract': 'pub fn captured_target(&self, pointer_id: PointerId) -> (result: Option<NodeId>) ensures result == captured_target_model(self, pointer_id), { proof { pointer_order_laws(); reveal(captured_target_model); }', 'spec_prelude': 'pub closed spec fn capture_present(router: &PointerRouter, id: PointerId) -> bool { router.captures@.contains_key(id) }\npub closed spec fn captured_target_model(router: &PointerRouter, id: PointerId) -> Option<NodeId> { if router.captures@.contains_key(id) { router.captures@[id].target } else { None } }', 'extra_annotations': [('pub fn has_capture(&self, pointer_id: PointerId) -> bool {', 'pub fn has_capture(&self, pointer_id: PointerId) -> (result: bool) ensures result == capture_present(self, pointer_id), { proof { pointer_order_laws(); reveal(capture_present); }')], 'function_count': 2, 'depends': ['pointer-order', 'pointer-router-type'], 'evidence': 'capture-query-kernel', 'scope': 'actual capture presence and original-target lookup over real PointerRouter map; no map mutation, hit testing, tracker consistency, cancellation or delivery proof', 'mutations': [('wrong-captured-target', '            capture.target\n', '            Some(NodeId(0))\n'), ('ignore-capture-presence', '        self.captures.contains_key(&pointer_id)\n', '        false\n')]},
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


KERNELS["ui-input-admission"] = {
    "source": "src/input_identity.rs", "marker": "UI INPUT ADMISSION", "property": "UI-INPUT-IDENTITY-001",
    "signature": ") -> bool {",
    "contract": """
) -> (result: bool)
    ensures result == (resource_allowed(resource_model(gate), caller_session, ticket.key)
        && current.target.0 == ticket.context.target.0
        && current.target_generation == ticket.context.target_generation
        && current.document == ticket.context.document
        && current.revision == ticket.context.revision),
{""",
    "depends": ["resource-gate", "node-id-type"], "evidence": "ui-input-admission",
    "scope": "UI identity admission predicate plus existing resource-gate dependency; trusted host current context and unique gate; not dispatch, authentication, callback cancellation or entire input lifecycle",
    "mutations": [("ignore-live-gate", "gate.allows(caller_session, ticket.key)", "true"),
                  ("ignore-document-version", "        && current.revision == ticket.context.revision\n}", "        && true\n}")],
}

KERNELS["ui-input-binding"] = {
    "source": "src/input_identity.rs", "marker": "UI INPUT BINDING", "property": "UI-INPUT-BINDING-001",
    "signature": "pub fn new(session: u64, slot: u64) -> Self {",
    "spec_prelude": """pub closed spec fn binding_model(binding: &UiInputBinding) -> ((u64,u64,u64,bool), Option<UiInputContext>) {
    (resource_model(&binding.gate), binding.current)
}
pub open spec fn binding_allowed(state: ((u64,u64,u64,bool), Option<UiInputContext>), caller: u64, ticket: UiInputTicket) -> bool {
    match state.1 {
        Some(current) => resource_allowed(state.0, caller, ticket.key)
            && current.target.0 == ticket.context.target.0
            && current.target_generation == ticket.context.target_generation
            && current.document == ticket.context.document
            && current.revision == ticket.context.revision,
        None => false,
    }
}""",
    "contract": """pub fn new(session: u64, slot: u64) -> (result: Self)
    ensures binding_model(&result) == ((session, slot, 0u64, false), None),
{ reveal(binding_model);""",
    "extra_annotations": [
        ("pub fn bind(&mut self, caller_session: u64, current: UiInputContext) -> Option<UiInputTicket> {", """pub fn bind(&mut self, caller_session: u64, current: UiInputContext) -> (result: Option<UiInputTicket>)
    ensures match result {
        Some(ticket) => caller_session == binding_model(old(self)).0.0
            && !binding_model(old(self)).0.3 && binding_model(old(self)).0.2 < u64::MAX
            && binding_model(final(self)).0.0 == binding_model(old(self)).0.0
            && binding_model(final(self)).0.1 == binding_model(old(self)).0.1
            && binding_model(final(self)).0.2 as int == binding_model(old(self)).0.2 as int + 1
            && binding_model(final(self)).0.3 && binding_model(final(self)).1 == Some(current)
            && ticket.context == current && resource_allowed(binding_model(final(self)).0, caller_session, ticket.key),
        None => binding_model(final(self)) == binding_model(old(self))
            && (caller_session != binding_model(old(self)).0.0 || binding_model(old(self)).0.3 || binding_model(old(self)).0.2 == u64::MAX),
    },
{ reveal(binding_model);"""),
        ("pub fn accepts(&self, caller_session: u64, ticket: UiInputTicket) -> bool {", """pub fn accepts(&self, caller_session: u64, ticket: UiInputTicket) -> (result: bool)
    ensures result == binding_allowed(binding_model(self), caller_session, ticket),
{ reveal(binding_model);"""),
        ("pub fn close(&mut self, caller_session: u64, ticket: UiInputTicket) -> bool {", """pub fn close(&mut self, caller_session: u64, ticket: UiInputTicket) -> (result: bool)
    ensures result == binding_allowed(binding_model(old(self)), caller_session, ticket),
        !result ==> binding_model(final(self)) == binding_model(old(self)),
        result ==> binding_model(final(self)) == ((binding_model(old(self)).0.0, binding_model(old(self)).0.1, binding_model(old(self)).0.2, false), None),
{ reveal(binding_model);"""),
        ("pub fn retire(&mut self) {", """pub fn retire(&mut self)
    ensures binding_model(final(self)) == ((binding_model(old(self)).0.0, binding_model(old(self)).0.1, u64::MAX, false), None),
{ reveal(binding_model);"""),
    ],
    "depends": ["ui-input-admission"], "evidence": "ui-input-binding",
    "scope": "persistent single-focus UI binding, exact vacant bind/reject frame, stale close rejection and permanent retirement; not event dispatch or cancellation of external tasks",
    "mutations": [("lose-published-context", "self.current = Some(current);", "self.current = None;"),
                  ("ignore-context-on-close", "if !self.accepts(caller_session, ticket)", "if false")],
}


KERNELS["ui-input-payload"] = {
    "source": "src/input_identity.rs", "marker": "UI INPUT PAYLOAD", "property": "UI-INPUT-PAYLOAD-001",
    "spec_prelude": """pub closed spec fn admitted_model<'a, T>(packet: &AdmittedUiInput<'a, T>) -> (&'a T, NodeId) {
    (packet.payload, packet.target)
}""",
    "signature": "pub fn payload(&self) -> &'a T {",
    "contract": """pub fn payload(&self) -> (result: &'a T)
    ensures result == admitted_model(self).0,
{ reveal(admitted_model);""",
    "extra_annotations": [
        ("pub fn target(&self) -> NodeId {", """pub fn target(&self) -> (result: NodeId)
    ensures result == admitted_model(self).1,
{ reveal(admitted_model);"""),
        (") -> Option<AdmittedUiInput<'a, T>> {", """) -> (result: Option<AdmittedUiInput<'a, T>>)
    ensures match result {
        Some(admitted) => binding_allowed(binding_model(self), caller_session, ticket)
            && admitted_model(&admitted).0 == payload && admitted_model(&admitted).1 == ticket.context.target,
        None => !binding_allowed(binding_model(self), caller_session, ticket),
    },
{ reveal(admitted_model);"""),
    ],
    "depends": ["ui-input-binding"], "evidence": "ui-input-payload",
    "scope": "parametric borrowed payload admission and immutable accessors; binding borrowed until last permit use; Rust borrow checker/compiler lifetime rules trusted; not asynchronous dispatch or payload content validation",
    "mutations": [("ignore-payload-admission", "if self.accepts(caller_session, ticket) {", "if true {")],
}


KERNELS["keyboard-admitted-input"] = {
    "source": "src/keyboard.rs", "marker": "KEYBOARD ADMITTED INPUT", "property": "UI-KEYBOARD-ADMISSION-001",
    "signature": ") -> Option<&'a T> {",
    "contract": """) -> (result: Option<&'a T>)
    ensures match result {
        Some(payload) => keyboard_focus_model(self).1 == Some(admitted_model(admitted).1)
            && payload == admitted_model(admitted).0,
        None => keyboard_focus_model(self).1 != Some(admitted_model(admitted).1),
    },
{""",
    "extra_annotations": [
        ("pub fn retire_input_binding(&mut self, binding: &mut UiInputBinding) -> Option<NodeId> {", """pub fn retire_input_binding(&mut self, binding: &mut UiInputBinding) -> (result: Option<NodeId>)
    ensures result == keyboard_focus_model(old(self)).1,
        keyboard_focus_model(final(self)).1 == None::<NodeId>,
        keyboard_focus_model(final(self)).0 == keyboard_focus_model(old(self)).0,
        binding_model(final(binding)) == ((binding_model(old(binding)).0.0, binding_model(old(binding)).0.1, u64::MAX, false), None),
{"""),
    ],
    "depends": ["ui-input-payload", "keyboard-focus"], "evidence": "keyboard-admitted-input",
    "scope": "real KeyboardRouter admitted payload target/focus filter plus persistent binding retirement before focus extraction; not raw legacy routing, editable-role/content checks, global target incarnation allocation or external task cancellation",
    "mutations": [("ignore-current-focus", "target.0 == admitted.target().0", "true"),
                  ("skip-binding-retirement", "        binding.retire();", "        // omitted binding retirement")],
}


KERNELS["ui-input-replace"] = {
    "source": "src/input_identity.rs", "marker": "UI INPUT REPLACE", "property": "UI-INPUT-REPLACE-001",
    "signature": ") -> Option<UiInputTicket> {",
    "contract": """) -> (result: Option<UiInputTicket>)
    ensures match result {
        Some(fresh) => binding_allowed(binding_model(old(self)), caller_session, ticket)
            && binding_model(old(self)).0.2 < u64::MAX
            && binding_model(final(self)).0.0 == binding_model(old(self)).0.0
            && binding_model(final(self)).0.1 == binding_model(old(self)).0.1
            && binding_model(final(self)).0.2 as int == binding_model(old(self)).0.2 as int + 1
            && binding_model(final(self)).0.3 && binding_model(final(self)).1 == Some(next)
            && fresh.context == next && resource_allowed(binding_model(final(self)).0, caller_session, fresh.key),
        None => binding_model(final(self)) == binding_model(old(self))
            && (!binding_allowed(binding_model(old(self)), caller_session, ticket) || ticket.key.generation == u64::MAX),
    },
{""",
    "depends": ["ui-input-binding"], "evidence": "ui-input-replace",
    "scope": "exact synchronous active binding rotation, generation no-wrap and rejection frame; next context trusted by host, no focus move or external cancellation",
    "mutations": [("lose-exhaustion-guard", "ticket.key.generation == u64::MAX || !self.accepts", "false || !self.accepts")],
}


KERNELS["focus-eligibility"] = {
    "source": "src/semantics.rs", "marker": "FOCUS ELIGIBILITY", "property": "UI-FOCUS-ELIGIBILITY-001",
    "signature": "pub fn can_focus(&self, node_id: NodeId) -> bool {",
    "contract": """pub fn can_focus(&self, node_id: NodeId) -> (result: bool)
    ensures result == focus_model(self).0.contains(node_id),
{ reveal(focus_model);""",
    "extra_annotations": [("        while index < self.order.len() {", """        while index < self.order.len()
            invariant index <= self.order.len(),
                forall|j: int| 0 <= j < index ==> self.order@[j].0 != node_id.0,
            decreases self.order.len() - index,
        {""")],
    "depends": ["focus-cleanup"], "evidence": "focus-eligibility",
    "scope": "read-only exact membership of current focus order; no semantic tree rebuild or host callbacks",
    "mutations": [("deny-found-target", "return true;", "return false;")],
}
KERNELS["keyboard-input-transfer"] = {
    "source": "src/keyboard.rs", "marker": "KEYBOARD INPUT TRANSFER", "property": "UI-KEYBOARD-TRANSFER-001",
    "signature": ") -> Option<UiInputTicket> {",
    "contract": """) -> (result: Option<UiInputTicket>)
    ensures result.is_some() == (keyboard_focus_model(old(self)).1 == Some(ticket.context.target)
        && keyboard_focus_model(old(self)).0.contains(next.target)
        && binding_allowed(binding_model(old(binding)), caller_session, ticket)
        && ticket.key.generation < u64::MAX),
        match result {
            Some(fresh) => keyboard_focus_model(final(self)).1 == Some(next.target)
                && keyboard_focus_model(final(self)).0 == keyboard_focus_model(old(self)).0
                && binding_model(final(binding)).0.0 == binding_model(old(binding)).0.0
                && binding_model(final(binding)).0.1 == binding_model(old(binding)).0.1
                && binding_model(final(binding)).0.2 as int == binding_model(old(binding)).0.2 as int + 1
                && binding_model(final(binding)).0.3 && binding_model(final(binding)).1 == Some(next)
                && fresh.context == next && resource_allowed(binding_model(final(binding)).0, caller_session, fresh.key),
            None => keyboard_focus_model(final(self)) == keyboard_focus_model(old(self))
                && binding_model(final(binding)) == binding_model(old(binding)),
        },
{""",
    "depends": ["ui-input-replace", "keyboard-focus", "focus-eligibility", "focus-select"], "evidence": "keyboard-input-transfer",
    "scope": "paired synchronous current-focus guard, target eligibility, binding rotation and focus commit with full rejection frame; host authenticates next context, no asynchronous or external cancellation guarantee",
    "mutations": [("skip-focus-commit", "let _ = self.focus.focus(next.target);", "// skipped focus commit")],
}


INPUT_OPEN_SIGNATURE = """pub fn begin_input_focus(
        &mut self,
        binding: &mut UiInputBinding,
        caller_session: u64,
        context: UiInputContext,
    ) -> Option<UiInputTicket> {"""
INPUT_CLOSE_SIGNATURE = """pub fn close_input_focus(
        &mut self,
        binding: &mut UiInputBinding,
        caller_session: u64,
        ticket: UiInputTicket,
    ) -> Option<UiInputTicket> {"""
KERNELS["keyboard-input-open-close"] = {
    "source": "src/keyboard.rs", "marker": "KEYBOARD INPUT OPEN CLOSE", "property": "UI-KEYBOARD-OPEN-CLOSE-001",
    "signature": INPUT_OPEN_SIGNATURE,
    "contract": INPUT_OPEN_SIGNATURE.replace(") -> Option<UiInputTicket> {", """) -> (result: Option<UiInputTicket>)
    ensures result.is_some() == (keyboard_focus_model(old(self)).1 == None::<NodeId>
        && keyboard_focus_model(old(self)).0.contains(context.target)
        && caller_session == binding_model(old(binding)).0.0 && !binding_model(old(binding)).0.3
        && binding_model(old(binding)).0.2 < u64::MAX),
        match result {
            Some(ticket) => keyboard_focus_model(final(self)).1 == Some(context.target)
                && keyboard_focus_model(final(self)).0 == keyboard_focus_model(old(self)).0
                && binding_model(final(binding)).0.0 == binding_model(old(binding)).0.0
                && binding_model(final(binding)).0.1 == binding_model(old(binding)).0.1
                && binding_model(final(binding)).0.2 as int == binding_model(old(binding)).0.2 as int + 1
                && binding_model(final(binding)).0.3 && binding_model(final(binding)).1 == Some(context)
                && ticket.context == context && resource_allowed(binding_model(final(binding)).0, caller_session, ticket.key),
            None => keyboard_focus_model(final(self)) == keyboard_focus_model(old(self))
                && binding_model(final(binding)) == binding_model(old(binding)),
        },
{"""),
    "extra_annotations": [(INPUT_CLOSE_SIGNATURE, INPUT_CLOSE_SIGNATURE.replace(") -> Option<UiInputTicket> {", """) -> (result: Option<UiInputTicket>)
    ensures result.is_some() == (keyboard_focus_model(old(self)).1 == Some(ticket.context.target)
        && binding_allowed(binding_model(old(binding)), caller_session, ticket)),
        match result {
            Some(owner) => owner == ticket && keyboard_focus_model(final(self)).1 == None::<NodeId>
                && keyboard_focus_model(final(self)).0 == keyboard_focus_model(old(self)).0
                && binding_model(final(binding)) == ((binding_model(old(binding)).0.0, binding_model(old(binding)).0.1, binding_model(old(binding)).0.2, false), None),
            None => keyboard_focus_model(final(self)) == keyboard_focus_model(old(self))
                && binding_model(final(binding)) == binding_model(old(binding)),
        },
{"""))],
    "depends": ["ui-input-binding", "keyboard-focus", "focus-eligibility", "focus-select"], "evidence": "keyboard-input-open-close",
    "scope": "paired first focus/binding publication and normal close returning fully qualified old owner, exact two-object reject frame; cancellation intent only, host delivery/auth/incarnation nonreuse separate",
    "mutations": [("omit-open-focus", "let _ = self.focus.focus(context.target);", "// omitted initial focus"),
                  ("omit-close-focus", "let _ = self.take_focus();", "// omitted normal focus clear")],
}


KERNELS["ui-composition-identity"] = {
    "source": "src/composition_identity.rs", "marker": "UI COMPOSITION IDENTITY", "property": "UI-COMPOSITION-IDENTITY-001",
    "signature": "fn same_input_ticket(a: UiInputTicket, b: UiInputTicket) -> bool {",
    "contract": """fn same_input_ticket(a: UiInputTicket, b: UiInputTicket) -> (result: bool)
    ensures result == (a == b),
{""",
    "spec_prelude": """pub closed spec fn composition_model(slot: &UiCompositionSlot) -> (u64, Option<UiCompositionToken>) {
    (slot.generation, slot.active)
}
pub open spec fn composition_active(state: (u64, Option<UiCompositionToken>), token: UiCompositionToken) -> bool {
    token.serial != 0 && token.serial == state.0 && state.1 == Some(token)
}""",
    "extra_annotations": [
        ("pub fn new() -> Self {", """pub fn new() -> (result: Self)
    ensures composition_model(&result) == (0u64, None),
{ reveal(composition_model);"""),
        ("pub fn begin(&mut self, binding: &UiInputBinding, caller: u64, owner: UiInputTicket) -> Option<UiCompositionToken> {", """pub fn begin(&mut self, binding: &UiInputBinding, caller: u64, owner: UiInputTicket) -> (result: Option<UiCompositionToken>)
    ensures match result {
        Some(token) => composition_model(old(self)).1 == None::<UiCompositionToken>
            && composition_model(old(self)).0 < u64::MAX
            && binding_allowed(binding_model(binding), caller, owner)
            && composition_model(final(self)).0 as int == composition_model(old(self)).0 as int + 1
            && token.owner == owner && token.serial == composition_model(final(self)).0
            && composition_model(final(self)).1 == Some(token),
        None => composition_model(final(self)) == composition_model(old(self))
            && (composition_model(old(self)).1.is_some() || composition_model(old(self)).0 == u64::MAX || !binding_allowed(binding_model(binding), caller, owner)),
    },
{ reveal(composition_model);"""),
        ("pub fn accepts(&self, binding: &UiInputBinding, caller: u64, token: UiCompositionToken) -> bool {", """pub fn accepts(&self, binding: &UiInputBinding, caller: u64, token: UiCompositionToken) -> (result: bool)
    ensures result == (composition_active(composition_model(self), token) && binding_allowed(binding_model(binding), caller, token.owner)),
{ reveal(composition_model);"""),
        ("pub fn cancel(&mut self, caller: u64, token: UiCompositionToken) -> bool {", """pub fn cancel(&mut self, caller: u64, token: UiCompositionToken) -> (result: bool)
    ensures result == (caller == token.owner.key.session && composition_active(composition_model(old(self)), token)),
        composition_model(final(self)).0 == composition_model(old(self)).0,
        result ==> composition_model(final(self)).1 == None::<UiCompositionToken>,
        !result ==> composition_model(final(self)) == composition_model(old(self)),
{ reveal(composition_model);"""),
        ("pub fn retire(&mut self) -> Option<UiCompositionToken> {", """pub fn retire(&mut self) -> (result: Option<UiCompositionToken>)
    ensures result == composition_model(old(self)).1,
        composition_model(final(self)) == (u64::MAX, None),
{ reveal(composition_model);"""),
    ],
    "depends": ["ui-input-binding"], "evidence": "ui-composition-identity",
    "scope": "UI preedit/candidate view correlation, active focus admission, nonwrapping composition serial, exact owner/serial cancel and retirement; host authentication/slot uniqueness and native cancellation delivery separate",
    "mutations": [("reset-serial-on-cancel", "                self.active = None;", "                self.active = None; self.generation = 0;"),
                  ("ignore-document-identity", "        && a.context.document == b.context.document", "        && true")],
}


KERNELS["composition-event-type"] = {"source": "src/input.rs", "marker": "COMPOSITION EVENT", "suffix": "TYPE", "signature": None, "depends": [], "internal_only": True}
KERNELS["ui-composition-range"] = {
    "source": "src/composition_identity.rs", "marker": "UI COMPOSITION RANGE", "property": "UI-COMPOSITION-RANGE-001",
    "signature": "pub fn valid_composition_range(document: &str, start: usize, end: usize) -> bool {",
    "spec_prelude": """use vstd::string::StringSliceAdditionalSpecFns;
pub open spec fn composition_range_valid(document: &str, start: usize, end: usize) -> bool {
    start <= end && end as int <= document.spec_bytes().len()
        && vstd::utf8::is_char_boundary(document.spec_bytes(), start as int)
        && vstd::utf8::is_char_boundary(document.spec_bytes(), end as int)
}""",
    "contract": """pub fn valid_composition_range(document: &str, start: usize, end: usize) -> (result: bool)
    ensures result == composition_range_valid(document, start, end),
{""",
    "extra_annotations": [(") -> Option<&'a CompositionEvent> {", """) -> (result: Option<&'a CompositionEvent>)
    ensures match result {
        Some(payload) => payload == event
            && composition_active(composition_model(self), token)
            && binding_allowed(binding_model(binding), caller, token.owner)
            && composition_range_valid(document, event.start, event.end),
        None => !(composition_active(composition_model(self), token)
            && binding_allowed(binding_model(binding), caller, token.owner)
            && composition_range_valid(document, event.start, event.end)),
    },
{""")],
    "depends": ["ui-composition-identity", "composition-event-type"], "evidence": "ui-composition-range",
    "scope": "actual existing CompositionEvent editor replacement byte range, valid UTF-8 scalar boundaries and current view/focus serial admission; vstd str len/is_char_boundary specifications trusted; not grapheme boundaries, preedit selection or document mutation",
    "mutations": [("allow-middle-scalar", "        && document.is_char_boundary(start) && document.is_char_boundary(end)", "        && true"),
                  ("skip-composition-owner", "if self.accepts(binding, caller, token)", "if true")],
}


KERNELS["preedit-event-type"] = {"source": "src/input.rs", "marker": "PREEDIT EVENT", "suffix": "TYPE", "signature": None, "depends": [], "internal_only": True}
KERNELS["ui-preedit-admission"] = {
    "source": "src/composition_identity.rs", "marker": "UI PREEDIT ADMISSION", "property": "UI-PREEDIT-ADMISSION-001",
    "signature": "pub fn valid_preedit_cursor(text: &str, cursor: Option<(usize, usize)>) -> bool {",
    "spec_prelude": """pub open spec fn preedit_bytes_valid(bytes: Seq<u8>, cursor: Option<(usize,usize)>) -> bool {
        match cursor {Some((start,end)) => start <= end && end as int <= bytes.len()
            && vstd::utf8::is_char_boundary(bytes,start as int)
            && vstd::utf8::is_char_boundary(bytes,end as int), None => true}
    }
    pub open spec fn preedit_cursor_valid(text: &str, cursor: Option<(usize,usize)>) -> bool {
        preedit_bytes_valid(text.spec_bytes(),cursor)
    }""",
    "contract": """pub fn valid_preedit_cursor(text: &str, cursor: Option<(usize, usize)>) -> (result: bool)
    ensures result == preedit_cursor_valid(text,cursor),
{""",
    "extra_annotations": [(") -> Option<&'a PreeditEvent> {", """) -> (result: Option<&'a PreeditEvent>)
    ensures match result {
        Some(payload) => payload == event
            && composition_active(composition_model(self), token)
            && binding_allowed(binding_model(binding), caller, token.owner)
            && preedit_bytes_valid(vstd::utf8::encode_utf8(event.text@),event.cursor),
        None => !(composition_active(composition_model(self), token)
            && binding_allowed(binding_model(binding), caller, token.owner)
            && preedit_bytes_valid(vstd::utf8::encode_utf8(event.text@),event.cursor)),
    },
{""")],
    "depends": ["ui-composition-range", "preedit-event-type"], "evidence": "ui-preedit-admission",
    "scope": "UI-specific preedit-local UTF-8 cursor/selection distinct from document replacement; None retained, borrowed current owner/serial admission. Desktop producer migration pending.",
    "mutations": [("deny-absent-cursor", "        None => true,", "        None => false,"),
                  ("skip-preedit-owner", "if self.accepts(binding, caller, token)\n            && valid_preedit_cursor", "if true\n            && valid_preedit_cursor")],
}


KERNELS["scroll-focus-types"] = {"source":"src/input.rs","marker":"SCROLL FOCUS","suffix":"TYPES","signature":None,"depends":["geometry-point-type"],"internal_only":True}
KERNELS["normalized-input-type"] = {"source":"src/input.rs","marker":"NORMALIZED INPUT","suffix":"TYPE","signature":None,"depends":["pointer-order","pointer-event-type","key-event-type","composition-event-type","scroll-focus-types"],"internal_only":True}
KERNELS["winit-ime-type"] = {"source":"verification/input-identity/winit-ime-type.rs","marker":"WINIT IME","suffix":"TYPE","signature":None,"depends":[],"internal_only":True}
KERNELS["desktop-ime-normalize"] = {
 "source":"src/desktop.rs","marker":"DESKTOP IME NORMALIZE","property":"UI-DESKTOP-IME-NORMALIZE-001",
 "signature":"pub fn normalize_ime(event: &Ime) -> Option<InputEvent> {",
 "contract":"""pub fn normalize_ime(event: &Ime) -> (result: Option<InputEvent>)
 ensures match event {
   Ime::Commit(text) => match result {Some(InputEvent::Text(copy)) => copy@ == text@, _ => false},
   Ime::Preedit(text,cursor) => match result {Some(InputEvent::Preedit{text:copy,cursor:position}) => copy@ == text@ && position == *cursor, _ => false},
   Ime::Enabled => matches!(result,Some(InputEvent::ImeEnabled(true))),
   Ime::Disabled => matches!(result,Some(InputEvent::ImeEnabled(false))),
 },
{""",
 "depends":["normalized-input-type","winit-ime-type"],"evidence":"desktop-ime-normalize",
 "scope":"Exact desktop normalize_ime body, all four pinned winit Ime variants, preserves Unicode text and optional local cursor; no document range inferred. Backend delivery and lifecycle disabled clearing not implemented.",
 "mutations":[("erase-cursor","cursor: *cursor,","cursor: None,"),("drop-commit","Ime::Commit(text) => Some(InputEvent::Text(text.clone())),","Ime::Commit(text) => None,")],
}


KERNELS["desktop-input-type"] = {"source":"src/desktop.rs","marker":"DESKTOP INPUT","suffix":"TYPE","signature":None,"depends":["normalized-input-type"],"internal_only":True,"spec_prelude":"use std::collections::BTreeSet; use std::time::Instant;\n#[verifier::external_type_specification] #[verifier::external_body] pub struct ExInstant(std::time::Instant);"}
KERNELS["desktop-ime-delegate"] = {
 "source":"src/desktop.rs","marker":"DESKTOP IME DELEGATE","property":"UI-DESKTOP-IME-DELEGATE-001",
 "signature":"pub fn ime(&mut self, event: &Ime) -> Option<InputEvent> {",
 "contract":"""impl DesktopInput {
    pub fn ime(&mut self, event: &Ime) -> (result: Option<InputEvent>)
    ensures *final(self) == *old(self), match event {
   Ime::Commit(text) => match result {Some(InputEvent::Text(copy)) => copy@ == text@, _ => false},
   Ime::Preedit(text,cursor) => match result {Some(InputEvent::Preedit{text:copy,cursor:position}) => copy@ == text@ && position == *cursor, _ => false},
   Ime::Enabled => matches!(result,Some(InputEvent::ImeEnabled(true))),
   Ime::Disabled => matches!(result,Some(InputEvent::ImeEnabled(false))),
 },
{""",
 "extra_annotations":[("        normalize_ime(event)\n    }","        normalize_ime(event)\n    }\n}")],
 "depends":["desktop-ime-normalize","desktop-input-type"],"evidence":"desktop-ime-delegate",
 "scope":"Actual DesktopInput::ime delegation to proved mapper; no translator state mutation, no cursor inference. Real event-loop routing outside this slice.",
 "mutations":[("drop-delegated-result","        normalize_ime(event)\n", "        None\n")],
}


KERNELS["normalized-preedit-admission"] = {
 "source":"src/composition_identity.rs","marker":"NORMALIZED PREEDIT ADMISSION","property":"UI-NORMALIZED-PREEDIT-001",
 "signature":"pub fn admit_normalized_preedit<'a>(",
 "contract":"pub fn admit_normalized_preedit<'a>(",
 "extra_annotations":[(") -> Option<&'a InputEvent> {", """) -> (result: Option<&'a InputEvent>)
 ensures match result {
   Some(payload) => payload == event && composition_active(composition_model(self),token)
     && binding_allowed(binding_model(binding),caller,token.owner) && normalized_preedit_valid(event),
   None => !(composition_active(composition_model(self),token)
     && binding_allowed(binding_model(binding),caller,token.owner) && normalized_preedit_valid(event)),
 },
{""")],
 "spec_prelude":"""pub open spec fn normalized_preedit_valid(event: &InputEvent) -> bool {
   match event {InputEvent::Preedit{text,cursor} => preedit_bytes_valid(vstd::utf8::encode_utf8(text@),*cursor), _=>false}
 }""",
 "depends":["ui-preedit-admission","desktop-ime-normalize"],"evidence":"normalized-preedit-admission",
 "scope":"Actual normalized host Preedit variant directly admitted with exact owner/serial and local UTF-8 cursor; other variants rejected. Does not invoke stateful desktop clock or external IME.",
 "mutations":[("ignore-normalized-owner","if self.accepts(binding, caller, token)\n                    && valid_preedit_cursor", "if true\n                    && valid_preedit_cursor"),
 ("accept-non-preedit","            _ => None,","            _ => Some(event),")],
}


KERNELS["ui-preedit-view"] = {
 "source":"src/composition_identity.rs","marker":"UI PREEDIT VIEW","property":"UI-PREEDIT-VIEW-001",
 "spec_prelude":"""pub closed spec fn preedit_view_model(view: &UiPreeditView) -> Option<UiPreeditFrame> { view.frame }
 pub open spec fn preedit_frame_matches(frame: UiPreeditFrame, caller:u64, token:UiCompositionToken) -> bool {
   caller == token.owner.key.session && token.serial != 0 && frame.token.serial == token.serial && frame.token.owner == token.owner
 }""",
 "signature":"pub fn new() -> Self {",
 "contract":"""pub fn new() -> (result: Self)
 ensures preedit_view_model(&result).is_none(),
 { reveal(preedit_view_model);""",
 "extra_annotations":[
 ("pub fn current(&self) -> Option<&UiPreeditFrame> {", """pub fn current(&self) -> (result: Option<&UiPreeditFrame>)
 ensures match result {Some(frame) => preedit_view_model(self)==Some(*frame), None=>preedit_view_model(self).is_none()},
 { reveal(preedit_view_model);"""),
 ("pub fn apply(&mut self, slot: &UiCompositionSlot, binding: &UiInputBinding, caller: u64, token: UiCompositionToken, event: InputEvent) -> bool {", """pub fn apply(&mut self, slot: &UiCompositionSlot, binding: &UiInputBinding, caller: u64, token: UiCompositionToken, event: InputEvent) -> (result: bool)
 ensures result == (composition_active(composition_model(slot),token) && binding_allowed(binding_model(binding),caller,token.owner) && normalized_preedit_valid(&event)),
 !result ==> preedit_view_model(final(self)) == preedit_view_model(old(self)),
 result ==> match event {InputEvent::Preedit{text,cursor} => match preedit_view_model(final(self)) {
   Some(frame) => frame.token == token && frame.text@ == text@ && frame.cursor == cursor, None=>false},_=>false},
 { reveal(preedit_view_model);"""),
 ("pub fn clear(&mut self, caller: u64, token: UiCompositionToken) -> bool {", """pub fn clear(&mut self, caller: u64, token: UiCompositionToken) -> (result: bool)
 ensures result == match preedit_view_model(old(self)) {Some(frame)=>preedit_frame_matches(frame,caller,token), None=>false},
 result ==> preedit_view_model(final(self)).is_none(),
 !result ==> preedit_view_model(final(self)) == preedit_view_model(old(self)),
 { reveal(preedit_view_model);""")],
 "function_count":4,"depends":["normalized-preedit-admission"],"evidence":"ui-preedit-view",
 "scope":"Actual owned UI preedit presentation storage: exact moved text/cursor/token, rejection frame, qualified clear after focus close and old-owner refusal; not document edits or pixel rendering/native cancellation.",
 "mutations":[("omit-view-replacement","self.frame = Some(UiPreeditFrame { token, text, cursor });","let _ = (token, text, cursor);"),
 ("ignore-view-serial","frame.token.serial != token.serial ||", "false ||")],
}


KERNELS["ui-preedit-cancel"] = {
 "source":"src/composition_identity.rs","marker":"UI PREEDIT CANCEL","property":"UI-PREEDIT-CANCEL-001",
 "signature":"pub fn cancel_preedit(&mut self, view: &mut UiPreeditView, caller: u64, token: UiCompositionToken) -> bool {",
 "contract":"""pub fn cancel_preedit(&mut self, view: &mut UiPreeditView, caller: u64, token: UiCompositionToken) -> (result: bool)
 ensures result == (caller == token.owner.key.session && composition_active(composition_model(old(self)),token)),
 composition_model(final(self)).0 == composition_model(old(self)).0,
 result ==> composition_model(final(self)).1.is_none(),
 !result ==> composition_model(final(self)) == composition_model(old(self)) && preedit_view_model(final(view)) == preedit_view_model(old(view)),
 result ==> match preedit_view_model(old(view)) {
   Some(frame) => if preedit_frame_matches(frame,caller,token) {preedit_view_model(final(view)).is_none()}
     else {preedit_view_model(final(view))==preedit_view_model(old(view))},
   None => preedit_view_model(final(view)).is_none(),
 },
{""",
 "depends":["ui-preedit-view"],"evidence":"ui-preedit-cancel",
 "scope":"Actual synchronous paired slot cancellation and exact owned-view clearing, rejection preserves both objects, mismatched later view preserved; no native task/IPC/pixel guarantee.",
 "mutations":[("omit-owned-clear","        let _ = view.clear(caller, token);","        let _ = (view, caller, token);"),
 ("clear-on-cancel-denial","if !self.cancel(caller, token) {\n            return false;", "if !self.cancel(caller, token) {\n            let _ = view.clear(caller, token); return false;")],
}


KERNELS["keyboard-preedit-close"] = {
 "source":"src/keyboard.rs","marker":"KEYBOARD PREEDIT CLOSE","property":"UI-KEYBOARD-PREEDIT-CLOSE-001",
 "signature":"pub fn close_input_preedit(&mut self, binding: &mut UiInputBinding, slot: &mut UiCompositionSlot, view: &mut UiPreeditView, caller: u64, token: UiCompositionToken) -> Option<UiCompositionToken> {",
 "contract":"""pub fn close_input_preedit(&mut self, binding: &mut UiInputBinding, slot: &mut UiCompositionSlot, view: &mut UiPreeditView, caller: u64, token: UiCompositionToken) -> (result: Option<UiCompositionToken>)
 ensures result.is_some() == (composition_active(composition_model(old(slot)),token) && binding_allowed(binding_model(old(binding)),caller,token.owner) && keyboard_focus_model(old(self)).1 == Some(token.owner.context.target)),
 match result {
   None => keyboard_focus_model(final(self)) == keyboard_focus_model(old(self)) && binding_model(final(binding)) == binding_model(old(binding)) && composition_model(final(slot)) == composition_model(old(slot)) && preedit_view_model(final(view)) == preedit_view_model(old(view)),
   Some(returned) => returned == token && keyboard_focus_model(final(self)) == (keyboard_focus_model(old(self)).0,None::<NodeId>)
     && binding_model(final(binding)) == ((binding_model(old(binding)).0.0,binding_model(old(binding)).0.1,binding_model(old(binding)).0.2,false),None::<UiInputContext>)
     && composition_model(final(slot)) == (composition_model(old(slot)).0,None::<UiCompositionToken>)
     && match preedit_view_model(old(view)) {Some(frame)=> if preedit_frame_matches(frame,caller,token) {preedit_view_model(final(view)).is_none()} else {preedit_view_model(final(view))==preedit_view_model(old(view))},None=>preedit_view_model(final(view)).is_none()},
 },
{""",
 "depends":["ui-preedit-cancel","keyboard-input-open-close"],"evidence":"keyboard-preedit-close",
 "scope":"Actual four-object synchronous active-preedit close: focus/binding/slot and qualified owned UI frame; reject frame for all four, return full old correlation, no native delivery/pixels/IPC.",
 "mutations":[("omit-preedit-cleanup","                let _ = slot.cancel_preedit(view, caller, token);","                let _ = (slot, view);"),
 ("cancel-before-focus-reject","        match self.close_input_focus(binding, caller, token.owner) {","        let _ = slot.cancel_preedit(view, caller, token);\n        match self.close_input_focus(binding, caller, token.owner) {")],
}


KERNELS["preedit-lifecycle-dispatch"] = {
 "source":"src/keyboard.rs","marker":"PREEDIT LIFECYCLE DISPATCH","property":"UI-PREEDIT-LIFECYCLE-001",
 "spec_prelude":"""pub open spec fn loses_ui_focus(event:&InputEvent)->bool {match event {InputEvent::Focus(f)=>!f.focused,_=>false}}
 pub open spec fn disables_ui_ime(event:&InputEvent)->bool {matches!(event,InputEvent::ImeEnabled(false))}""",
 "signature":"pub fn handle_preedit_lifecycle(&mut self, binding: &mut UiInputBinding, slot: &mut UiCompositionSlot, view: &mut UiPreeditView, caller: u64, token: UiCompositionToken, event: &InputEvent) -> Option<UiCompositionToken> {",
 "contract":"""pub fn handle_preedit_lifecycle(&mut self, binding: &mut UiInputBinding, slot: &mut UiCompositionSlot, view: &mut UiPreeditView, caller: u64, token: UiCompositionToken, event: &InputEvent) -> (result: Option<UiCompositionToken>)
 ensures result.is_some() == (composition_active(composition_model(old(slot)),token) && binding_allowed(binding_model(old(binding)),caller,token.owner) && (disables_ui_ime(event) || (loses_ui_focus(event) && keyboard_focus_model(old(self)).1==Some(token.owner.context.target)))),
 match result {
 None=>keyboard_focus_model(final(self))==keyboard_focus_model(old(self)) && binding_model(final(binding))==binding_model(old(binding)) && composition_model(final(slot))==composition_model(old(slot)) && preedit_view_model(final(view))==preedit_view_model(old(view)),
 Some(returned)=>returned==token && composition_model(final(slot))==(composition_model(old(slot)).0,None::<UiCompositionToken>)
  && (if disables_ui_ime(event) {keyboard_focus_model(final(self))==keyboard_focus_model(old(self)) && binding_model(final(binding))==binding_model(old(binding))}
      else {keyboard_focus_model(final(self))==(keyboard_focus_model(old(self)).0,None::<NodeId>) && binding_model(final(binding))==((binding_model(old(binding)).0.0,binding_model(old(binding)).0.1,binding_model(old(binding)).0.2,false),None::<UiInputContext>)})
  && match preedit_view_model(old(view)) {Some(frame)=>if preedit_frame_matches(frame,caller,token) {preedit_view_model(final(view)).is_none()} else {preedit_view_model(final(view))==preedit_view_model(old(view))},None=>preedit_view_model(final(view)).is_none()},
 },
{""",
 "depends":["keyboard-preedit-close"],"evidence":"preedit-lifecycle-dispatch",
 "scope":"Actual active-composition UI lifecycle dispatch: Focus(false) closes four objects; ImeEnabled(false) cancels slot/view while preserving keyboard focus/binding; all other/rejected events frame unchanged. Host-qualified token/authentication, no external delivery.",
 "mutations":[("disable-closes-focus","if !slot.accepts(binding, caller, token) { return None; }\n                let _ = slot.cancel_preedit(view, caller, token);", "if !slot.accepts(binding, caller, token) { return None; }\n                let _ = self.take_focus(); let _ = slot.cancel_preedit(view, caller, token);"),
 ("ignore-disabled-owner","if !slot.accepts(binding, caller, token) { return None; }", "if false { return None; }")],
}


KERNELS["composition-current-token"] = {
 "source":"src/composition_identity.rs","marker":"COMPOSITION CURRENT TOKEN","property":"UI-COMPOSITION-CURRENT-001",
 "signature":"pub fn current_token(&self) -> Option<UiCompositionToken> {",
 "contract":"""pub fn current_token(&self) -> (result: Option<UiCompositionToken>)
 ensures result == composition_model(self).1,
 { reveal(composition_model);""",
 "depends":["ui-composition-identity"],"evidence":"composition-current-token",
 "scope":"Exact read-only correlation metadata, not admission or authentication",
 "mutations":[("hide-current-token","self.active }","None }")],
}
KERNELS["input-lifecycle-dispatch"] = {
 "source":"src/keyboard.rs","marker":"INPUT LIFECYCLE DISPATCH","property":"UI-INPUT-LIFECYCLE-001",
 "signature":"pub fn handle_input_lifecycle(&mut self, binding: &mut UiInputBinding, slot: &mut UiCompositionSlot, view: &mut UiPreeditView, caller: u64, owner: UiInputTicket, event: &InputEvent) -> Option<(UiInputTicket, Option<UiCompositionToken>)> {",
 "contract":"""pub fn handle_input_lifecycle(&mut self, binding: &mut UiInputBinding, slot: &mut UiCompositionSlot, view: &mut UiPreeditView, caller: u64, owner: UiInputTicket, event: &InputEvent) -> (result: Option<(UiInputTicket, Option<UiCompositionToken>)>)
 ensures result.is_some() == (binding_allowed(binding_model(old(binding)),caller,owner) && match composition_model(old(slot)).1 {
 Some(token)=>composition_active(composition_model(old(slot)),token) && binding_allowed(binding_model(old(binding)),caller,token.owner) && (disables_ui_ime(event) || (loses_ui_focus(event) && keyboard_focus_model(old(self)).1==Some(token.owner.context.target))),
 None=>preedit_view_model(old(view)).is_none() && loses_ui_focus(event) && keyboard_focus_model(old(self)).1==Some(owner.context.target),
 }),
 match result {
 None=>keyboard_focus_model(final(self))==keyboard_focus_model(old(self)) && binding_model(final(binding))==binding_model(old(binding)) && composition_model(final(slot))==composition_model(old(slot)) && preedit_view_model(final(view))==preedit_view_model(old(view)),
 Some((returned,cancellation))=>returned==owner && match cancellation {
 None=>composition_model(old(slot)).1.is_none() && composition_model(final(slot))==composition_model(old(slot)) && preedit_view_model(old(view)).is_none() && preedit_view_model(final(view)).is_none()
  && keyboard_focus_model(final(self))==(keyboard_focus_model(old(self)).0,None::<NodeId>) && binding_model(final(binding))==((binding_model(old(binding)).0.0,binding_model(old(binding)).0.1,binding_model(old(binding)).0.2,false),None::<UiInputContext>),
 Some(token)=>composition_model(old(slot)).1==Some(token) && composition_model(final(slot))==(composition_model(old(slot)).0,None::<UiCompositionToken>)
  && (if disables_ui_ime(event) {keyboard_focus_model(final(self))==keyboard_focus_model(old(self)) && binding_model(final(binding))==binding_model(old(binding))} else {keyboard_focus_model(final(self))==(keyboard_focus_model(old(self)).0,None::<NodeId>) && binding_model(final(binding))==((binding_model(old(binding)).0.0,binding_model(old(binding)).0.1,binding_model(old(binding)).0.2,false),None::<UiInputContext>)})
  && match preedit_view_model(old(view)) {Some(frame)=>if preedit_frame_matches(frame,caller,token) {preedit_view_model(final(view)).is_none()} else {preedit_view_model(final(view))==preedit_view_model(old(view))},None=>preedit_view_model(final(view)).is_none()},
 },
 },
{""",
 "depends":["preedit-lifecycle-dispatch","composition-current-token"],"evidence":"input-lifecycle-dispatch",
 "scope":"Actual unified UI lifecycle for idle/composing owner, four-object reject frame, idle lost-focus close, preserved focus on IME disabled, defensive orphan-view refusal; not external cancellation delivery.",
 "mutations":[("forge-idle-close","Some(closed) => Some((closed, None)),","Some(closed) => None,"),
 ("ignore-lifecycle-owner","if !binding.accepts(caller, owner) { return None; }","if false { return None; }")],
}


# Read-only native facade projection. Third-party field types are opaque: no constructor,
# renderer, destructor or upstream method is assumed or verified by this proof.
KERNELS["native-view-type"] = {
    "source": "src/native_adapter/native_view.rs", "marker": "NATIVE VIEW", "suffix": "TYPE",
    "signature": None, "depends": ["ui-view-slot", "ui-composition-identity"], "internal_only": True,
    "spec_prelude": "pub mod blitz_dom { #[verifier::external_body] pub struct NodeId {} }\n#[verifier::external_body] pub struct BaseDocument {}",
}
KERNELS["native-component-version"] = {
    "source": "src/native_adapter/native_view.rs", "marker": "NATIVE COMPONENT VERSION",
    "property": "UI-NATIVE-COMPONENT-VERSION-001",
    "signature": "pub(crate) fn component_version(&self) -> Option<u64> {",
    "contract": """pub(crate) fn component_version(&self) -> (result: Option<u64>)
        ensures result == native_component_version_model(self),
    { proof { reveal(native_component_version_model); }""",
    "spec_prelude": "pub closed spec fn native_component_version_model(view: &NativeTextView) -> Option<u64> { if view.document.owned_value().is_some() { Some(view.component_version) } else { None } }",
    "depends": ["native-view-type"], "evidence": "native-component-version-kernel", "function_count": 1,
    "scope": "Exact native facade getter and exact field declaration; open slot returns stored version, closed slot returns None. Upstream payload/NodeId are opaque third-party types; no renderer/destructor/other facade method proof.",
    "mutations": [("wrong-native-version", "Some(self.component_version)", "Some(0)"), ("expose-closed-version", "if self.document.is_open()", "if true")],
}


KERNELS["native-activation-admission"] = {
    "source": "src/native_adapter/native_semantics.rs", "marker": "NATIVE ACTIVATION ADMISSION",
    "property": "UI-NATIVE-ACTIVATION-ADMISSION-001",
    "signature": "pub(crate) fn accepts_activation(current_version: u64, expected_version: u64, can_activate: bool) -> bool {",
    "contract": """pub(crate) fn accepts_activation(current_version: u64, expected_version: u64, can_activate: bool) -> (result: bool)
        ensures result == (current_version == expected_version && can_activate),
    {""",
    "depends": [], "evidence": "native-activation-admission-kernel", "function_count": 1,
    "scope": "Exact production semantic action eligibility predicate; owner-admitted snapshot is a precondition of caller composition, not proven here; no callback, backend, host authentication or replay proof.",
    "mutations": [("ignore-version", "    current_version == expected_version && can_activate\n", "    can_activate\n"), ("ignore-enabled-role", "    current_version == expected_version && can_activate\n", "    current_version == expected_version\n")],
}


KERNELS["component-update-types"] = {
    "source": "src/native_adapter/component_contract.rs", "marker": "COMPONENT UPDATE", "suffix": "TYPES",
    "signature": None, "depends": [], "internal_only": True,
}
KERNELS["component-update-support"] = {
    "source": "src/native_adapter/component_contract.rs", "marker": "COMPONENT UPDATE SUPPORT",
    "property": "UI-COMPONENT-UPDATE-SUPPORT-001",
    "signature": "pub(crate) fn supports_component_update(button: bool, update: ComponentUpdate<'_>) -> bool {",
    "contract": """pub(crate) fn supports_component_update(button: bool, update: ComponentUpdate<'_>) -> (result: bool)
        ensures result == match update {
            ComponentUpdate::ButtonCaption(_) | ComponentUpdate::ButtonDisabled(_) => button,
            ComponentUpdate::Appearance(appearance) => appearance.font_size_px > 0,
            _ => true,
        },
    {""",
    "depends": ["component-update-types"], "evidence": "component-update-support-kernel", "function_count": 1,
    "scope": "Actual role/value predicate and exact own update declarations used by batch preflight; not batch loop, version, owner, native mutation or resolve proof.",
    "mutations": [("allow-text-button-update", "\n        ComponentUpdate::ButtonCaption(_) | ComponentUpdate::ButtonDisabled(_) => button,", "\n        ComponentUpdate::ButtonCaption(_) | ComponentUpdate::ButtonDisabled(_) => true,"), ("allow-zero-font", "\n        ComponentUpdate::Appearance(appearance) => appearance.font_size_px > 0,", "\n        ComponentUpdate::Appearance(appearance) => true,")],
}


KERNELS["scene-work-capacity"] = {
    "source":"src/native_adapter/native_scene.rs", "marker":"SCENE WORK CAPACITY",
    "property":"UI-SCENE-WORK-CAPACITY-001",
    "signature":"pub(crate) fn scene_work_fits(prepared:usize,pending:usize,incoming:usize)->bool {",
    "contract":"""pub(crate) fn scene_work_fits(prepared:usize,pending:usize,incoming:usize)->(result:bool)
        ensures result == (prepared as int + pending as int + incoming as int <= 4096),
    {""",
    "depends":[], "evidence":"scene-work-capacity-kernel", "function_count":1,
    "scope":"Exact production count predicate used before traversal pushes and patch preparation; no text/font/backend allocation or full traversal invariant proof.",
    "mutations":[("ignore-pending","    incoming<=4096-prepared-pending", "    incoming<=4096-prepared"),("off-by-one","    incoming<=4096-prepared-pending", "    incoming<4096-prepared-pending")],
}


KERNELS["scene-update-admission"] = {
    "source":"src/native_adapter/native_scene.rs", "marker":"SCENE UPDATE ADMISSION",
    "property":"UI-SCENE-UPDATE-ADMISSION-001",
    "signature":"pub(crate) fn accepts_scene_update(owner:u64,caller:u64,current:u64,expected:u64,next:u64,count:usize,open:bool)->bool {",
    "contract":"""pub(crate) fn accepts_scene_update(owner:u64,caller:u64,current:u64,expected:u64,next:u64,count:usize,open:bool)->(result:bool)
        ensures result == (owner == caller && open && current == expected && current < u64::MAX
            && next as int == current as int + 1 && count > 0 && count <= 4096),
    {""",
    "depends":["scene-work-capacity","ui-update-next"], "evidence":"scene-update-admission-kernel", "function_count":1,
    "scope":"Exact production patch admission and real reused version/count predicates; no native mutation loop or host authentication proof.",
    "mutations":[("ignore-scene-owner","if owner!=caller || !open", "if false || !open"),("ignore-next-version","Some(successor)=>next==successor", "Some(successor)=>true")],
}


KERNELS["source-insert-index"] = {
    "source":"src/native_adapter/native_scene.rs", "marker":"SOURCE INSERT INDEX",
    "property":"UI-SOURCE-INSERT-INDEX-001",
    "signature":"pub(crate) fn source_insert_index(index:usize,children:usize)->usize {",
    "contract":"""pub(crate) fn source_insert_index(index:usize,children:usize)->(result:usize)
        ensures result == (if index <= children {index} else {children}), result <= children,
    {""",
    "depends":[], "evidence":"source-insert-index-kernel", "function_count":1,
    "scope":"Exact adapter index clamp matching existing Scene::apply_one min semantics; not tree placement/identity/constructor proof.",
    "mutations":[("unclamped-index","    index.min(children)","    index"),("always-append","    index.min(children)","    children")],
}


KERNELS["native-host-context"] = {
    "source":"src/native_adapter/native_view.rs","marker":"NATIVE HOST CONTEXT",
    "signature":"pub(crate) fn host_context_matches(&self, caller: u64, expected: UiInputContext) -> bool {",
    "contract":"""pub(crate) fn host_context_matches(&self, caller: u64, expected: UiInputContext) -> (result:bool)
        ensures result == (self.session==caller && self.context.target.0==expected.target.0 && self.context.target_generation==expected.target_generation && self.context.document==expected.document && self.context.revision==expected.revision),
    {""",
    "depends":["native-view-type","ui-view-owner"],"internal_only":False,
    "property":"UI-NATIVE-HOST-CONTEXT-001","evidence":"native-host-context-kernel","function_count":1,
    "scope":"Exact NativeTextView read-only owner/context equality only; no backend, authentication or lifecycle effect proof.",
    "mutations":[("ignore-caller","matches_ui_view_owner(self.session,self.context,caller,expected)","matches_ui_view_owner(self.session,self.context,self.session,expected)"),
        ("ignore-revision","matches_ui_view_owner(self.session,self.context,caller,expected)","matches_ui_view_owner(self.session,self.context,caller,self.context)")],
}
KERNELS["text-display-type"] = {
    "source":"src/native_adapter/facade.rs","marker":"TEXT DISPLAY","suffix":"TYPE",
    "signature":None,"depends":["native-host-context","native-component-version"],"internal_only":True,
    "spec_prelude":"pub(crate) mod native_view { pub(crate) use super::NativeTextView; }",
}
KERNELS["text-display-version"] = {
    "source":"src/native_adapter/facade.rs","marker":"TEXT DISPLAY VERSION",
    "property":"UI-TEXT-DISPLAY-VERSION-001",
    "signature":"pub fn update_version(&self,owner:u64,context:UiInputContext)->Option<u64> {",
    "contract":"""pub fn update_version(&self,owner:u64,context:UiInputContext)->(result:Option<u64>)
        ensures result==text_display_version_model(self,owner,context),
    { proof { reveal(text_display_version_model); }""",
    "spec_prelude":"pub closed spec fn text_display_version_model(view:&TextDisplayView,owner:u64,context:UiInputContext)->Option<u64> {if view.inner.session==owner && view.inner.context.target.0==context.target.0 && view.inner.context.target_generation==context.target_generation && view.inner.context.document==context.document && view.inner.context.revision==context.revision {native_component_version_model(&view.inner)} else {None}}",
    "depends":["text-display-type"],"evidence":"text-display-version-kernel","function_count":1,
    "scope":"Actual public read-only owner/context/version projection plus actual own helper and slot getter; unused backend payloads opaque, no construction/render/other facade proof.",
    "mutations":[("ignore-owner-context","if !self.inner.host_context_matches(owner,context)","if false"),("wrong-version","        self.inner.component_version()","        Some(0)")],
}

KERNELS["native-view-close"] = {
 "source":"src/native_adapter/native_view.rs","marker":"NATIVE VIEW CLOSE",
 "signature":"pub(crate) fn close(&mut self) -> bool {",
 "contract":"""pub(crate) fn close(&mut self) -> (result:bool)
 ensures result==old(self).document.owned_value().is_some(),
    final(self).document.owned_value().is_none(),
    composition_model(&final(self).preedit)==(u64::MAX,None::<UiCompositionToken>),
    native_metadata_same(old(self),final(self)),
 { proof { reveal(native_metadata_same); }""",
 "spec_prelude":"pub closed spec fn native_metadata_same(a:&NativeTextView,b:&NativeTextView)->bool {a.component_version==b.component_version && a.session==b.session && a.context==b.context && a.content_node==b.content_node && a.text_node==b.text_node && a.text_element==b.text_element && a.preedit_node==b.preedit_node && a.preedit_element==b.preedit_element}",
 "depends":["native-view-type"],"internal_only":True,
}
KERNELS["text-display-close"] = {
 "source":"src/native_adapter/facade.rs","marker":"TEXT DISPLAY CLOSE","property":"UI-TEXT-DISPLAY-CLOSE-001",
 "signature":"pub fn close(&mut self,owner:u64,context:UiInputContext)->bool {",
 "contract":"""pub fn close(&mut self,owner:u64,context:UiInputContext)->(result:bool)
 ensures text_display_close_post(old(self),final(self),owner,context,result),
 { proof { reveal(text_display_close_post); reveal(text_display_owner_model); }""",
 "spec_prelude":"pub closed spec fn text_display_close_post(a:&TextDisplayView,b:&TextDisplayView,owner:u64,context:UiInputContext,result:bool)->bool {result==(text_display_owner_model(a,owner,context) && a.inner.document.owned_value().is_some()) && if text_display_owner_model(a,owner,context) {b.inner.document.owned_value().is_none() && composition_model(&b.inner.preedit)==(u64::MAX,None::<UiCompositionToken>) && native_metadata_same(&a.inner,&b.inner)} else {*a==*b}}\npub closed spec fn text_display_owner_model(view:&TextDisplayView,owner:u64,context:UiInputContext)->bool {view.inner.session==owner && view.inner.context.target.0==context.target.0 && view.inner.context.target_generation==context.target_generation && view.inner.context.document==context.document && view.inner.context.revision==context.revision}",
 "depends":["text-display-type","native-view-close"],"evidence":"text-display-close-kernel","function_count":1,
 "scope":"Actual owner-admitted public close, preedit correlation retirement and owned Option closure; logical frame only, no backend destructor/service ACK proof.",
 "mutations":[("ignore-owner-context","if !self.inner.host_context_matches(owner,context)","if false"),("skip-retirement","        self.inner.close()","        false")],
}

KERNELS["scene-semantics-type"]={"source":"src/semantics.rs","marker":"SCENE SEMANTICS","suffix":"TYPE","signature":None,"depends":["role-type"],"internal_only":True}
KERNELS["native-scene-type"]={"source":"src/native_adapter/native_scene.rs","marker":"NATIVE SCENE","suffix":"TYPE","signature":None,"depends":["ui-view-slot","node-id-type","scene-semantics-type"],"internal_only":True,"spec_prelude":"use std::collections::BTreeMap; pub mod blitz_dom {#[verifier::external_body] pub struct NodeId {}} #[verifier::external_body] pub struct BaseDocument {}"}
KERNELS["native-scene-close"]={
 "source":"src/native_adapter/native_scene.rs","marker":"NATIVE SCENE CLOSE",
 "signature":"pub(crate) fn close(&mut self,caller:u64)->bool {",
 "contract":"""pub(crate) fn close(&mut self,caller:u64)->(result:bool)
 ensures result==(caller==old(self).owner && old(self).document.owned_value().is_some()),
    if caller==old(self).owner {final(self).document.owned_value().is_none()} else {final(self).document.owned_value()==old(self).document.owned_value()},
    native_scene_metadata_same(old(self),final(self)),
 {proof {reveal(native_scene_metadata_same);}""",
 "spec_prelude":"pub closed spec fn native_scene_metadata_same(a:&NativeScene,b:&NativeScene)->bool {a.owner==b.owner && a.scene_version==b.scene_version && a.node_count==b.node_count && a.parents@==b.parents@ && a.children@==b.children@ && a.displayed@==b.displayed@ && a.semantics@==b.semantics@ && a.nodes@==b.nodes@}",
 "depends":["native-scene-type"],"internal_only":True,
}
KERNELS["scene-host-type"]={"source":"src/native_adapter/facade.rs","marker":"SCENE HOST","suffix":"TYPE","signature":None,"depends":["native-scene-close"],"internal_only":True,"spec_prelude":"pub(crate) mod native_scene {pub(crate) use super::NativeScene;}"}
KERNELS["scene-host-close"]={
 "source":"src/native_adapter/facade.rs","marker":"SCENE HOST CLOSE","property":"UI-SCENE-HOST-CLOSE-001",
 "signature":"pub fn close(&mut self,owner:u64)->bool {",
 "contract":"""pub fn close(&mut self,owner:u64)->(result:bool)
 ensures scene_host_close_post(old(self),final(self),owner,result),
 {proof {reveal(scene_host_close_post);}""",
 "spec_prelude":"pub closed spec fn scene_host_close_post(a:&SceneHostView,b:&SceneHostView,owner:u64,result:bool)->bool {result==(owner==a.inner.owner && a.inner.document.owned_value().is_some()) && (if owner==a.inner.owner {b.inner.document.owned_value().is_none()} else {b.inner.document.owned_value()==a.inner.document.owned_value()}) && native_scene_metadata_same(&a.inner,&b.inner)}",
 "depends":["scene-host-type"],"evidence":"scene-host-close-kernel","function_count":1,
 "scope":"Actual Scene host/native close and exact map/semantic declarations; logical owned slot retirement plus metadata preservation, no map algorithm, rendering, destructor or host cleanup proof.",
 "mutations":[("ignore-owner","caller==self.owner && self.document.close()","self.document.close()"),("skip-close","self.inner.close(owner)","false")],
}

KERNELS["cpu-frame-bytes"] = {
    "source":"src/native_adapter/cpu_budget.rs", "marker":"CPU FRAME BYTES",
    "property":"UI-CPU-FRAME-BYTES-001",
    "signature":"pub(crate) fn cpu_frame_bytes(width:u32,height:u32,budget:usize)->Option<usize> {",
    "contract":"""pub(crate) fn cpu_frame_bytes(width:u32,height:u32,budget:usize)->(result:Option<usize>)
        ensures match result {
            Some(bytes)=>width>0 && height>0 && width<=65535 && height<=65535
                && bytes as int==width as int*height as int*4 && bytes<=budget && bytes<=4294967295,
            None=>width==0 || height==0 || width>65535 || height>65535
                || width as int*height as int*4>budget as int || width as int*height as int*4>4294967295,
        },
    {""",
    "extra_annotations":[("    let bytes=(width as u64)*(height as u64)*4;",
        "    assert(width as int*height as int<=4294836225) by (nonlinear_arith) requires 0<=width as int, width<=65535, 0<=height as int, height<=65535;\n    let bytes=(width as u64)*(height as u64)*4;")],
    "depends":[], "evidence":"cpu-frame-bytes-kernel", "function_count":1,
    "scope":"Exact packed CPU size/budget guard shared by text and scene; not allocation/scratch or backend correctness.",
    "mutations":[("wrong-pixel-stride","(height as u64)*4","(height as u64)*3"),
        ("ignore-host-budget","bytes>budget as u64 || bytes>4294967295","bytes>4294967295")],
}

KERNELS["capture-cancel-route-type"]={"source":"src/native_adapter/capture_policy.rs","marker":"CAPTURE CANCEL ROUTE","suffix":"TYPE","signature":None,"depends":[],"internal_only":True}
KERNELS["capture-cancel-policy"]={
    "source":"src/native_adapter/capture_policy.rs","marker":"CAPTURE CANCEL POLICY","property":"UI-CAPTURE-CANCEL-POLICY-001",
    "signature":"pub(crate) fn capture_cancel_route(session:u64,current:UiInputContext,caller:u64,origin:UiInputContext,deliver:bool,open:bool)->CaptureCancelRoute {",
    "contract":"""pub(crate) fn capture_cancel_route(session:u64,current:UiInputContext,caller:u64,origin:UiInputContext,deliver:bool,open:bool)->(result:CaptureCancelRoute)
    ensures match result {
        CaptureCancelRoute::Reject=>!(session==caller && current.target.0==origin.target.0 && current.target_generation==origin.target_generation && current.document==origin.document),
        CaptureCancelRoute::HostOnly=>session==caller && current.target.0==origin.target.0 && current.target_generation==origin.target_generation && current.document==origin.document && (!deliver || !open),
        CaptureCancelRoute::Current=>session==caller && current.target.0==origin.target.0 && current.target_generation==origin.target_generation && current.document==origin.document && deliver && open && current.revision==origin.revision,
        CaptureCancelRoute::Successor=>session==caller && current.target.0==origin.target.0 && current.target_generation==origin.target_generation && current.document==origin.document && deliver && open && current.revision!=origin.revision,
    },
    {""",
    "depends":["capture-cancel-route-type","ui-snapshot-admission","ui-view-owner"],"evidence":"capture-cancel-policy-kernel","function_count":1,
    "scope":"Exact cancellation route decision with real root identity predicates; native/host effects excluded.",
    "mutations":[("bypass-lineage","if !accepts_ui_display_snapshot(session,origin,caller,origin,current)","if false"),
        ("notify-successor","else {CaptureCancelRoute::Successor}","else {CaptureCancelRoute::Current}")],
}

KERNELS["scene-snapshot-identity"]={
    "source":"src/native_adapter/native_scene.rs","marker":"SCENE SNAPSHOT IDENTITY",
    "property":"UI-SCENE-SNAPSHOT-IDENTITY-001",
    "signature":"pub(crate) fn snapshot_identity(&self,caller:u64)->Option<(u64,usize)> {",
    "contract":"""pub(crate) fn snapshot_identity(&self,caller:u64)->(result:Option<(u64,usize)>)
    ensures result == (if caller==self.owner && self.document.owned_value().is_some() {Some((self.scene_version,self.node_count))} else {None}),
    {""",
    "depends":["native-scene-type"],"evidence":"scene-snapshot-identity-kernel","function_count":1,
    "scope":"Actual read-only metadata projection after owner/open admission; not correctness of cached node_count or native tree correspondence.",
    "mutations":[("ignore-owner","if caller!=self.owner || !self.document.is_open()","if !self.document.is_open()"),
        ("ignore-retirement","if caller!=self.owner || !self.document.is_open()","if caller!=self.owner")],
}

KERNELS["scene-host-identity"]={
    "source":"src/native_adapter/facade.rs","marker":"SCENE HOST IDENTITY",
    "property":"UI-SCENE-HOST-IDENTITY-001",
    "signature":"pub fn snapshot_identity(&self,owner:u64)->Option<(u64,usize)> {",
    "contract":"""pub fn snapshot_identity(&self,owner:u64)->(result:Option<(u64,usize)>)
    ensures result == scene_host_identity_model(self,owner),
    { proof {reveal(scene_host_identity_model);}""",
    "spec_prelude":"pub closed spec fn scene_host_identity_model(view:&SceneHostView,owner:u64)->Option<(u64,usize)> {if owner==view.inner.owner && view.inner.document.owned_value().is_some() {Some((view.inner.scene_version,view.inner.node_count))} else {None}}",
    "depends":["scene-host-type","scene-snapshot-identity"],"evidence":"scene-host-identity-kernel","function_count":1,
    "scope":"Actual public wrapper plus actual stored metadata getter; not authentication or count/tree correspondence.",
    "mutations":[("ignore-owner","self.inner.snapshot_identity(owner)","self.inner.snapshot_identity(self.inner.owner)"),
        ("always-absent","self.inner.snapshot_identity(owner)","None")],
}

KERNELS["native-button-role"]={
    "source":"src/native_adapter/native_scene.rs","marker":"NATIVE BUTTON ROLE",
    "signature":"pub(crate) fn role_uses_native_button(role:Role)->bool {",
    "contract":"""pub(crate) fn role_uses_native_button(role:Role)->(result:bool)
    ensures result == (match role {Role::Button=>true,_=>false}),
    {""",
    "depends":["role-type"],"internal_only":True,
}
KERNELS["semantic-role-tag"]={
    "source":"src/native_adapter/native_scene.rs","marker":"SEMANTIC ROLE TAG",
    "property":"UI-SEMANTIC-ROLE-TAG-001",
    "signature":"pub(crate) fn semantic_role_update_preserves_tag(current:Role,next:Role)->bool {",
    "contract":"""pub(crate) fn semantic_role_update_preserves_tag(current:Role,next:Role)->(result:bool)
    ensures result == ((match current {Role::Button=>true,_=>false}) == (match next {Role::Button=>true,_=>false})),
    {""",
    "depends":["native-button-role"],"evidence":"semantic-role-tag-kernel","function_count":1,
    "scope":"Exact tag-preservation decision used by semantic preflight; constructor mapping, maps and whole mutation frame unproved.",
    "mutations":[("wrong-native-class","matches!(role,Role::Button)","false"),
        ("invert-tag-relation","role_uses_native_button(current)==role_uses_native_button(next)","role_uses_native_button(current)!=role_uses_native_button(next)")],
}

KERNELS["native-backend-error-type"]={"source":"src/backend.rs","marker":"NATIVE BACKEND ERROR","suffix":"TYPE","signature":None,"depends":[],"internal_only":True}
KERNELS["native-headless-backend-type"]={"source":"src/backend.rs","marker":"NATIVE HEADLESS BACKEND","suffix":"TYPE","signature":None,"depends":["canvas-import","native-backend-error-type"],"internal_only":True}
KERNELS["native-backend-frame-import"]={
    "source":"src/backend.rs","marker":"NATIVE BACKEND FRAME IMPORT","property":"UI-NATIVE-BACKEND-FRAME-001",
    "signature":"fn import_native_frame(&mut self,width:u32,height:u32,pixels:Vec<u8>)->Result<(),BackendError> {",
    "contract":"""fn import_native_frame(&mut self,width:u32,height:u32,pixels:Vec<u8>)->(result:Result<(),BackendError>)
    ensures match result {
        Ok(()) => width > 0 && height > 0 && pixels.len() as int == width as int * height as int * 4
            && canvas_model(&final(self).canvas) == (width,height,pixels@,0int),
        Err(_) => canvas_model(&final(self).canvas) == canvas_model(&old(self).canvas)
            && final(self).canvas.clip_stack@ == old(self).canvas.clip_stack@
            && (width == 0 || height == 0 || pixels.len() as int != width as int * height as int * 4),
    },
    {""",
    "depends":["native-headless-backend-type"],"evidence":"native-backend-frame-import-kernel","function_count":1,
    "scope":"Actual native Headless backend frame replacement method; exact success pixels/dimensions/empty clipping and rejection preserves prior frame; not renderer, trait dispatch or presentation.",
    "mutations":[("skip-frame-replacement","self.canvas=canvas;Ok(())","let _=canvas;Ok(())"),
        ("swap-frame-dimensions","Canvas::from_rgba8(width,height,pixels)","Canvas::from_rgba8(height,width,pixels)")],
}

def extract(root, name):
    config = KERNELS[name]
    source = (root / config["source"]).read_text()
    suffix = config.get("suffix", "KERNEL")
    begin = "// BEGIN VERIFIED " + config["marker"] + " " + suffix + "\n"
    end = "// END VERIFIED " + config["marker"] + " " + suffix
    if source.count(begin) != 1 or source.count(end) != 1:
        raise ValueError(name + ": markers must occur exactly once")
    body = source.split(begin, 1)[1].split(end, 1)[0]
    if config["signature"] is None:
        return body, body
    if body.count(config["signature"]) != 1:
        raise ValueError(name + ": signature changed; review contract")
    annotated = body.replace(config["signature"], config["contract"], 1)
    for signature, contract in config.get("extra_annotations", []):
        if annotated.count(signature) != 1:
            raise ValueError(name + ": additional signature changed; review contract")
        annotated = annotated.replace(signature, contract, 1)
    return body, annotated


def dependency_order(name):
    ordered = []
    def visit(item):
        if item in ordered:
            return
        for dependency in KERNELS[item]["depends"]:
            visit(dependency)
        ordered.append(item)
    visit(name)
    return ordered


def summary(output):
    match = re.search(r"verification results:: (\d+) verified, (\d+) errors", output)
    return (int(match[1]), int(match[2])) if match else None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verus", required=True)
    parser.add_argument("--kernel", choices=[name for name, item in KERNELS.items()
                                             if not item.get("internal_only")], default="version")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    config = KERNELS[args.kernel]
    evidence = root / "evidence" / config["evidence"]
    evidence.mkdir(parents=True, exist_ok=True)
    # Remove stale success before extraction or tool launch can fail.
    # 在提取或调用工具可能失败之前清除旧成功报告。
    (evidence / "report.json").unlink(missing_ok=True)
    ordered = dependency_order(args.kernel)
    if "winit-ime-type" in ordered:
        cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
        candidates = list((cargo_home / "registry/src").glob("*/winit-0.30.13/src/event.rs"))
        if len(candidates) != 1:
            raise ValueError("exact pinned winit source required for Ime declaration mapping")
        upstream = candidates[0].read_text()
        declaration = "pub enum Ime {" + upstream.split("pub enum Ime {",1)[1].split("\n}",1)[0] + "\n}"
        snapshot = (root / "verification/input-identity/winit-ime-type.rs").read_text()
        if declaration not in snapshot:
            raise ValueError("Ime dependency snapshot differs from actual pinned winit declaration")
        (evidence / "dependency-mapping.json").write_text(json.dumps({"package":"winit", "version":"0.30.13", "actual_source":str(candidates[0]), "source_sha256":sha(candidates[0].read_bytes()), "declaration_sha256":sha(declaration.encode()), "declaration_exact":True},indent=2)+"\n")
    extracted = {name: extract(root, name) for name in ordered}
    specs = "\n".join(KERNELS[name].get("spec_prelude", "") for name in ordered)
    proof = "use vstd::prelude::*;\nverus! {\n" + specs + "\n" + "\n".join(
        extracted[name][1] for name in ordered) + "\n}\n"
    proof_path = evidence / "kernel.rs"
    proof_path.write_text(proof)
    version = subprocess.run([args.verus, "--version"], capture_output=True, text=True, timeout=30)
    # The pinned release bundles Z3; record any explicit solver override as well.
    # 固定发行包带 Z3；同时记录显式求解器覆盖，避免证据隐瞒工具环境。
    solver_path = Path(os.environ.get("VERUS_Z3_PATH", str(Path(args.verus).resolve().parent / "z3")))
    solver = subprocess.run([str(solver_path), "--version"], capture_output=True, text=True, timeout=30)
    options = ["--crate-type", "lib"]
    command = [args.verus, *options, str(proof_path)]
    result = subprocess.run(command, capture_output=True, text=True, timeout=60)
    output = result.stdout + result.stderr
    (evidence / "verus.log").write_text(output)
    positive = summary(output)
    controls = []
    for label, pattern, replacement in config["mutations"]:
        if proof.count(pattern) != 1:
            raise ValueError("review negative control: " + label)
        path = evidence / ("negative_" + label.replace("-", "_") + ".rs")
        path.write_text(proof.replace(pattern, replacement, 1))
        negative = subprocess.run([args.verus, *options, str(path)],
                                  capture_output=True, text=True, timeout=60)
        negative_output = negative.stdout + negative.stderr
        (evidence / ("negative-" + label + ".log")).write_text(negative_output)
        stats = summary(negative_output)
        # An option/parser/type error is not a valid rejected counterexample.
        # 参数、解析或类型错误不算有效反例，必须实际触发后置条件验证失败。
        rejected = (negative.returncode != 0 and stats is not None and stats[1] > 0
                    and "postcondition not satisfied" in negative_output)
        controls.append({"name": label, "rejected": rejected, "exit_code": negative.returncode})
    passed = (version.returncode == 0 and solver.returncode == 0 and result.returncode == 0 and positive is not None
              and positive[0] >= sum(KERNELS[name].get("function_count", int(KERNELS[name]["signature"] is not None)) for name in ordered)
              and positive[1] == 0
              and all(control["rejected"] for control in controls))
    source_files = {KERNELS[name]["source"] for name in ordered}
    report = {
        "property": config["property"], "status": "proved" if passed else "not_verified",
        "scope": config["scope"], "integer_model": "Verus default integer/usize model; not a cross-platform execution test",
        "source_sha256": sha((root / config["source"]).read_bytes()),
        "sources": {name: sha((root / name).read_bytes()) for name in sorted(source_files)},
        "kernel_sha256": sha(extracted[args.kernel][0].encode()),
        "dependencies": {name: sha(extracted[name][0].encode()) for name in ordered[:-1]},
        "proof_sha256": sha(proof.encode()), "runner_sha256": sha(Path(__file__).read_bytes()),
        "verus": version.stdout + version.stderr, "command": command, "exit_code": result.returncode,
        "solver": {"path": str(solver_path), "version": solver.stdout + solver.stderr,
                   "explicit_override": os.environ.get("VERUS_Z3_PATH")},
        "verified_obligations": positive[0] if positive else 0,
        "negative_controls": controls,
        "negative_control_rejected": all(control["rejected"] for control in controls),
        "trust_boundary": ["Verus/vstd and SMT solver", "Rust/Verus integer, references and slice semantics",
                           "signature annotation/extraction script", "Rust compiler and target execution"],
    }
    (evidence / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(output, end="")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
