# Structural navigation and bounded operand selection

Date: 2026-10-15. Static analysis only. The original application, GUI, CLR,
COM, and screenshots were not used. The Rust behavior described below is an
independent implementation.

## Target and retained evidence

The target is the installed Mathcad Prime 12.0.0.1
`Ptc.EquationEditor.dll`, SHA-256
`d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`,
MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441`.

- REA selected-type parent evidence:
  `ev_fd77c469a4e79dcea831eee774b27cd5a793ad0c324dea2b4b314892b5ec0dee`.
- REA complete selected-type summary:
  `ev_cdc797d09464ff0f20074a8c2ebfea98f322d8ba1f5e0283ef5d07d64c2ee4db`.
- REA managed-method page containing all 331 selected `EquationModel`
  methods: `ev_3f23fa7a0d4a8f7bcf4b4f3269308389e04cfb85b12dc055d2044f6cb821db2f`.

The selected type has complete metadata coverage. REA decoded file-backed CIL
without loading or executing the assembly. Tokens are build-local and only
identify methods together with the DLL hash and MVID above. The `#US` heap has
reported nonstandard terminal flags; those string issues do not affect the
method identities used here.

The shipped `Doc/en-US/keyboard_shortcuts.pdf`, extracted as ignored
`artifacts/rea/editor/keyboard-shortcuts.txt`, has SHA-256
`db4cdf1f913cfad20075b3f6354085183a22bde588900709c12ca06c065b3a4f`.

## Observations

The following are static shipped-artifact observations, not runtime behavior.

| Command surface | Token, body offset, raw CIL SHA-256 | Observation |
| --- | --- | --- |
| Move one position | `MoveLeft` `0x06000153`, 31912, `800b464c011831858dc94ed8e68596cb02c18a847e5dd3f170b8525a40a13770`; `MoveRight` `0x06000155`, 31938, `a3d4d04663b52f4cca16e6aa47e6dffbae7c862f280b7a8c2e0ce6c228f93054` | The public model has distinct ordinary left/right commands. |
| Move by term | `MoveLeftByTerm` `0x06000154`, 31925, `8ceca1d5765d7f7a37ab6dd5d4becca200b0444c11f75761275e1b2bf3445575`; `MoveRightByTerm` `0x06000156`, 31951, `f1eb95b18db8ff79a4991fdb51ac728a736ef93f26655ed9b4be837bc0bfed6c` | Term movement is a separate command path, rather than repeated character movement. |
| Current-expression extremes | `MoveToCurrentExpressionStart` `0x0600016f`, 32774, `97c73fd29230b053bd403f5cfc2676214b5005378d40c019cce60603a231ffa3`; `MoveToCurrentExpressionEnd` `0x06000170`, 32787, `f3440551617abf611e4edf4fb267b5e92b9005b19066c93a05ef7b69e259a710` | Expression start/end are distinct from region start/end and atom-local navigation. |
| Directional selection | `SelectLeft` `0x0600015f`, 32094, `afde42977fc56aaa2dcbc95cc9c025c4ca01b893da21256d480c50a46518e910`; `SelectRight` `0x06000160`, 32107, `e3aa48fd658baa2c6a087419edfe3fb6039bd528c7caf9b4b0a61cd6219c44aa` | The model exposes directional selection commands independently of motion. |
| Selection extremes | `SelectToStart` `0x06000162`, 32133, `58b37e7d973eb9cb7302abf5aff82ff7a7543b477a52d99b22fe5c0d85ca93de`; `SelectToEnd` `0x06000161`, 32120, `e7e68fcaf8a7abbfdd57276a1b1021ad0cf203a037e43046c64eba7c56b58446` | Start/end selection also has dedicated entry points. |

The keyboard document says Home and End move to the expression beginning and
end. Ctrl+Left and Ctrl+Right move to the left/right of a selected expression,
or to the beginning/end of the selected word. It also documents ordinary arrow
movement between placeholders. These are documentation facts; this pass did not
execute the product to establish precedence-sensitive paths or visual timing.

The selected `EquationModel` methods delegate through editing-mode virtual
methods. Some base-mode selection implementations are empty. Therefore the API
separation above is observed, while exact behavior for every mode and node type
remains unknown.

## Independent Rust behavior

`Editor::move_to_expression_extreme` now moves Home/End across nested operands
to the first or last editable leaf of the formula. It keeps UTF-16 coordinates,
clears a selection, and does not create an undo record.

Ctrl+Left/Right now treats a selected subtree as one term and an atomic value as
one word. It collapses a selected subtree to the requested structural side or
moves an atomic caret to its word boundary. Ordinary left/right remains
character and structure navigation.

Shift+Left/Right retains character selection inside an atom. Once the entire
atom is selected, another shifted arrow promotes it to a whole structural
operand; subsequent presses use the existing scope history to grow through
parents. `Editor::select_subtree_from` preserves the entry side so `/`, `^`, and
infix insertion put the selected operand in the corresponding slot. This
promotion sequence is a project policy constrained by the current tree model,
not a claim that every Prime transition is identical.

Deleting or typing over the promoted subtree uses the existing placeholder and
stable-ID command path. Undo restores the original tree, caret, structural
selection, and direction.

## Verification and unknowns

Headless Rust tests cover nested Home/End movement, no navigation undo entry,
Ctrl+Left/Right word and selected-expression boundaries, UTF-16 character
selection promotion, parent-scope growth, direction-sensitive fraction
insertion, replacement, and exact undo restoration. A focused egui test sends
the actual Home, End, and Ctrl+Arrow key events without opening a window.

Partial adjacent operand ranges, precedence-sensitive term boundaries, matrix
rectangle selection, Shift+Home/End ranges, selection contraction from the
keyboard, and Prime's exact highlight geometry remain unrepresented or
unverified. A whole subtree is the explicit current bound; those missing states
must not be described as recovered parity.
