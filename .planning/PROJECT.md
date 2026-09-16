# PommeMC Client

## Vision

Pomme is a lightweight, high-performance Minecraft client written in Rust that speaks the latest Minecraft protocol (26.2) natively while maintaining complete, bidirectional compatibility across 1.20–1.21 release protocols (763 through 776). It embeds SteelMC as its zero-friction singleplayer integrated server.

## Core Value

A flawless vanilla Minecraft survival experience that boots in milliseconds, renders smoothly with high render distances, connects seamlessly to both modern and legacy servers, and has zero compromises on gameplay, physics, combat, or audio parity.

## Architecture

1. **Direct-to-Latest Wire Translation**: Inbound packets from legacy wire versions (1.20.1/763 through 1.21.11/774) are translated directly into the latest 26.2 memory layout before dispatch, avoiding intermediate version hops or ViaVersion pipelines. Static registry IDs and dynamic item/block states are remapped at the packet boundary.
2. **Authoritative Client State**: Player abilities, attributes, and physics parameters are driven strictly by server packets rather than local assumptions, preventing desyncs across game modes and version boundaries.
3. **Generated Data Substrate**: Block shapes, collision boxes, physics properties, sounds, and registry tables are generated directly from verified decompiled reference sources rather than hand-coded approximations.
4. **Modern Vulkan/Metal Renderer**: Per-chunk mesh pipelines, texture atlasing with guttering, translucent face sorting, and specialized entity/block-entity pipelines.
5. **Integrated Server Substrate (SteelMC)**: Integrated singleplayer worlds run on SteelMC, providing natural mob spawning, AI behaviors, world generation, and progression sequences.

## Key Decisions

- Direct-to-latest translation: No intermediate protocol states. Every older version translates straight to 26.2 in one pass.
- GPL-3.0-or-later licensing: Strict compliance with open-source boundaries. No decompiled proprietary Mojang code is checked in; behavior is verified through automated black-box acceptance suites.
- Concrete acceptance ledger: Every behavioral requirement and compatibility marker must be backed by runnable unit/integration tests and recorded in `acceptance/gap-ledger.json`.
