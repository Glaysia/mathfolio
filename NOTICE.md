# Mathcraft licensing notices

Copyright (C) 2026 Mathcraft contributors.

SPDX-License-Identifier: AGPL-3.0-only

Except for third-party materials identified by their own licenses or notices,
the original Mathcraft source code, scripts and documentation in this repository
are licensed under the GNU Affero General Public License, version 3 only.

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU Affero General Public License as published by the Free
Software Foundation, version 3 of the License.

This program is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE. See the GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License along
with this program. If not, see <https://www.gnu.org/licenses/>. The full license
text is in [LICENSE](LICENSE).

## Bundled third-party source and fonts

- The vendored `egui-winit` source retains its `MIT OR Apache-2.0` licensing.
  Preserve [LICENSE-MIT](vendor/egui-winit/LICENSE-MIT),
  [LICENSE-APACHE](vendor/egui-winit/LICENSE-APACHE) and the provenance and patch
  notes in [UPSTREAM.json](vendor/egui-winit/UPSTREAM.json) and
  [MATHCRAFT.md](vendor/egui-winit/MATHCRAFT.md).
- The original vendored `egui-wgpu`, `egui_software_backend` and `constify`
  sources retain their MIT OR Apache-2.0 licenses. Preserve their bundled
  license texts and provenance in [egui-wgpu](vendor/egui-wgpu/MATHCRAFT.md)
  and [egui-software](vendor/egui-software/MATHCRAFT.md). The new Mathcraft
  Windows surface module and its regression tests are AGPL-3.0-only, as
  identified in their source headers.
- STIX Two Math and Text fonts retain the SIL Open Font License 1.1. Preserve
  [OFL.txt](assets/fonts/stix/OFL.txt) and
  [SOURCE.json](assets/fonts/stix/SOURCE.json).
- The optional WASM CJK font Nanum Gothic retains SIL Open Font License 1.1;
  preserve [OFL.txt](assets/fonts/nanum/OFL.txt) and
  [SOURCE.json](assets/fonts/nanum/SOURCE.json). It is an external host asset,
  not an addition to the native executable's embedded fonts.
- egui's embedded default fonts retain their individual licenses, including
  Ubuntu Font License, SIL OFL, and Hack/emoji font notices. Copies and exact
  dependency provenance are in [egui-default](assets/fonts/egui-default/SOURCE.json).
- Nord palette values are credited to Sven Greb and the Nord project (MIT).
  Preserve [LICENSE-MIT](assets/themes/nord/LICENSE-MIT) and
  [SOURCE.json](assets/themes/nord/SOURCE.json). The semantic UI mapping is
  Mathcraft's implementation; no Nord CSS runtime or web engine is bundled.
- Other dependencies retain the licenses and copyright notices supplied by
  their respective authors. This project-level notice does not relicense
  third-party materials, including interoperability reference fixtures.

The project license is the unmodified GNU AGPLv3. No project-specific exceptions
or additional restrictions are added. See [the GNU explanation of the Affero
GPL](https://www.gnu.org/licenses/why-affero-gpl.en.html) for its network-use
source-offer requirement.
