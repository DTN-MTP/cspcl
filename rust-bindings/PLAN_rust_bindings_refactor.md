# Atomic Plan: Make the Rust CSPCL Bindings Operational, Minimal, and Hardy-Ready

This plan is intentionally commit-oriented and may be adjusted during implementation if missing constraints, C API drift, or integration details are discovered.

## Summary

Refactor the Rust bindings in one large atomic change that:

- makes `cspcl-sys` build the local C implementation instead of assuming symbols already exist
- realigns the Rust FFI layer with the current `cspcl.h` / `cspcl.c` surface
- simplifies the safe `cspcl` API for Hardy by centering it on explicit send/receive handles rather than raw mutable struct access

## Big Commit

- make `cspcl-sys` operational as a vendored sys crate with stub fallback and explicit real-libcsp integration hooks
- remove safe-API drift against the C layer and stop exposing raw `cspcl_t` internals in normal use
- replace the monolithic mutable wrapper with a small bootstrap handle plus split `Sender` / `Receiver` handles
- update Rust docs/examples so they match the actual API and lifecycle

## Technical Direction

- treat `cspcl-sys` as the authoritative raw layer generated from the current header and linked to the compiled local C objects
- keep the public safe API limited to configuration, initialization, sending, receiving, address translation, and typed errors
- let outbound sending use the C connection pool instead of forcing `&mut self` on all operations
- serialize blocking receive calls per instance while keeping send and receive handles easy to pass into Hardy abstractions

## Verification

- workspace Rust builds must resolve against the local `cspcl-sys` crate instead of the crates.io copy
- the generated bindings and native objects must expose the same symbols the safe crate calls
- docs/examples must describe the new split-handle model
- a second planning pass can break this atomic refactor into smaller follow-up commits if needed
