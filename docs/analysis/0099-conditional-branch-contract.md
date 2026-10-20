# 조건 분기 체인·BODY·원본 조판 계약 — 2026-10-19

[0096](0096-program-control-authoring-contract.md)의 프로그램 저작 조사를 이어간다. 제품 코드와 `docs/spec`은 변경하지 않았다. 설치 Prime 12.0.0.1의 기존 REA managed metadata/CIL과 동봉 설정 파일을 정적으로 읽었다. GUI, 스크린샷, 원본 앱, CLR, COM 실행은 사용하지 않았다. 이 문서는 원본 계약의 근거이며 구현 완료·런타임 동등성 보고서가 아니다.

## 증거 신원과 재현

설치 기준 경로는 `C:/Program Files/PTC/Mathcad Prime 12.0.0.1/`다. 기존 `artifacts/rea/editor/{equation,control}-{members,il}.json`, `context-labels-deep/` 및 중간 조사 `artifacts/rea/program-branch-next-contract-20261019.md`를 재사용했다. `feedback/label-cycle-inspect.py`는 설치 파일 전체 SHA와 선택한 메서드의 원시 IL SHA를 REA metadata와 대조한 뒤 해독한다. 같은 token이라도 E와 C의 메서드는 다르다.

| 기호 | 어셈블리 | SHA-256 / MVID / REA Evidence |
| --- | --- | --- |
| E | `Ptc.EquationEditor.dll` | `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223` / `d8627dca-3258-470c-b4d1-c00bbe2f2441` / `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf` |
| C | `Ptc.Controls.EquationControl.dll` | `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1` / `cf1d7cb5-c73f-4618-8e6b-b83959d13b98` / `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349` |
| B | `C:/Program Files/dotnet/shared/Microsoft.NETCore.App/8.0.29/System.Private.CoreLib.dll` | `8c01f630bb23c2fb142e6559cdcdf147105625defb36107019d4ab50d8854cd3` / `32073012-e02c-4cb4-991b-03bb9c2a40fe` / Dictionary `ev_95cbe8a380bd96438567aa4b978dc15d5f0f1b9f58c264110c142afe379b49a5`, Enumerator `ev_049019a5cc2963401fcd53be9773a2c47c5165d98ed96546a5a19e4c9d58ccf7` |

B는 이번에 정확한 두 declaring type을 선택하여 REA로 조사했다. REA의 nested type 명칭은 `Enumerator`이며 다른 Enumerator들도 선택되므로 실제 Dictionary nested TypeDef `0x02000995`와 NestedClass 관계를 별도로 대조했다. `feedback/conditional-corelib-selected.json`은 선택한 REA method 행, `conditional-corelib-decode.py`는 해당 파일의 raw IL SHA를 검증하는 보조 해독기다. REA 요약 조회에서 `Dictionary\`2+Enumerator`는 unmatched였으며 그 요청이 nested 메서드를 제공했다고 주장하지 않는다.

## IF는 한 개의 pair-nary 노드다

E `DefaultDescriptorTable.f` `0x06000bbb`, IL SHA `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a`, `IL_468c..4807`:

- 첫 subelement `IF_THEN`, 중간 `ELSE_IF`, 마지막 `ELSE`, alternate 중간 `ALSO_IF`를 한 IF descriptor에 등록한다.
- 기본 두 child, primal child 1, precedence 290이며 실제 child 배열은 `[test0, body0, test1, body1, …, optional_else_body]`다.
- alternate 상태는 **노드 전체의 bool 하나**다. 각 branch에 서로 다른 종류를 저장하는 모델이 아니다.
- IF_THEN/ELSE_IF/ALSO_IF는 각각 두 child와 primal 1, ELSE는 한 child와 primal 0이며 모두 parent pair IF를 선언한다.

E writer `0x0600119d`는 첫 pair를 `<if>` 바로 아래에 쓰고, 추가 pair를 global alternate flag에 따라 `<elseif>` 또는 `<alsoif>`로 쓴다(`IL_0059..006d`). 마지막 홀수 child는 `<else>`다(`0031..0057`). pair writer `0x060011a4`는 test와 body를 `<test>`/`<then>`으로 감싼다. 따라서 다음은 canonical 구조 예시다. namespace prefix는 예시이며 Math50 namespace 자체가 계약이다.

```xml
<ml:if xmlns:ml="http://schemas.mathsoft.com/math50">
  <ml:test><ml:real>1</ml:real></ml:test>
  <ml:then><ml:program><ml:real>2</ml:real></ml:program></ml:then>
  <ml:elseif>
    <ml:test><ml:real>0</ml:real></ml:test>
    <ml:then><ml:program><ml:real>3</ml:real></ml:program></ml:then>
  </ml:elseif>
  <ml:else><ml:program><ml:real>4</ml:real></ml:program></ml:else>
</ml:if>
```

E reader `0x06001270`는 첫 pair와 추가 pair를 같은 flat 배열에 모은다. `<alsoif>`가 하나라도 있으면 global alternate flag가 true가 된다(`IL_006b..0085`, `00c2..00c4`). 따라서 **원본 reader를 거친 mixed elseif/alsoif 입력은 다음 저장 시 전부 alsoif가 되는 정규화**를 수행한다. 이것은 원본 authoring에서 자유로운 mixed chain을 허용한다거나 raw numerical engine이 같은 mixed XML을 직접 허용한다는 증거는 아니다.

## BODY와 PROGRAM의 차이 및 scalar body 수용

**BODY는 내부 descriptor 이름이며 native `<body>`라는 새 wrapper를 만들지 않는다.** E transform 등록 `0x06001167 IL_0aa8..0ab8`은 BODY에 XML 이름 `program`을 지정한다. closure 생성 `0x06001174` → closure `0x060011ac` → 일반 element writer `0x06001175`는 이 이름으로 element를 만들고 children을 변환한다. `TransformChildNodes` `0x0600116d`는 `ConvertChildNodes` → AppendChildNodes다. unary visual-only child를 푸는 일반 규칙이 `0x0600116e`에 있으나 BODY의 자체 dispatch를 무조건 제거하는 규칙이 아니다.

IF pair helper `0x060011a4`와 else writer는 body child를 **generic transform**에 넘긴다. 이 helper 자체가 무조건 program으로 감싸는 것은 아니다. 반대 방향 pair reader `0x06001273 IL_0000..0043`도 `<test>`/`<then>`의 FirstChild를 각각 generic reader `0x06001243`으로 넘긴다. else도 동일하게 FirstChild를 읽는다. 그러므로 scalar expression이 직접 들어 있는 then/else를 원본 reader가 프로그램 wrapper 부족만으로 거부하지 않는다.

그 다음 descriptor repair가 body를 canonical 내부 구조로 만든다.

| 대상 slot | E `0x06000bbb` 범위 | 조치 |
| --- | --- | --- |
| IF child1 | `47b7..47ff` | PROGRAM → BODY descriptor 변경; 그 밖의 child → BODY parent 생성 |
| IF_THEN child1 | `48cb..4913` | 동일 |
| ELSE child0 | `49e3..4a2b` | 동일 |
| ELSE_IF child1 | `4b03..4b4b` | 동일 |
| ALSO_IF child1 | `4c23..4c6b` | 동일 |

`ReplaceChildDescriptor` handler E `0x06000590`는 기존 child의 descriptor만 바꾸고 children을 유지한다. `ReparentChild` handler `0x0600058d`는 새 BODY parent를 만들고 기존 child를 slot에 넣으며 tracked navigator도 새 parent로 갱신한다. 따라서 native import에서 `<then><real>2</real></then>`을 한 statement의 Program body로 수용한 뒤 `<then><program><real>2</real></program></then>`으로 canonical 저장하는 정책은 원본 reader+repair의 증거와 일치한다. placeholder body도 같은 종류의 repair 대상이다. 단 body 안의 모든 nested program을 재귀적으로 flatten하는 근거는 없다. 명시적으로 중첩된 statement 구조는 보존해야 한다.

이번 재구성의 AST가 BODY를 별도 종류로 갖지 않고 body 위치의 Program으로 표현한다면 위의 canonical subset은 표현할 수 있다. 그러나 body 위치 밖의 PROGRAM과 BODY의 모든 편집/조판 차이까지 같아졌다고 주장하면 안 된다.

### Native import와 raw calculation의 경계

원본 native reader를 모방하는 import 경로에서는 scalar body의 단일 statement 감싸기와 any-alsoif의 전체 alternate 정규화를 함께 적용하는 것이 일관된다. **계산은 그 정규화한 editor AST를 통해 진행**할 수 있다. 이미 존재하는 raw calculation XML parser의 mixed rejection을 authoring 작업 중 함께 바꾸어야 한다는 근거는 없다. raw parser의 엄격한 경계는 명시적으로 남기고 별도 원본 engine 계약 조사 후 변경한다. 이 권고는 관찰한 reader/writer 계약에서 도출한 구현 정책이며 원본 계산 엔진을 실행한 결과가 아니다.

## Ctrl+J `if`의 첫 descriptor는 기본 설치 구성에서 IF다

[0096](0096-program-control-authoring-contract.md)의 Normal mode, IdentifierNodeInfo, HasNoLabel(null/None), exact whole formatted identifier 비교 조건은 그대로다. E `a8.bq` `0x0600059e IL_0024..0060`은 DefaultDescriptorTable dictionary를 열거하여 OperatorText가 같은 **첫 항목**을 선택한다. 첫 매치의 삽입 실패 후 다음 동명 descriptor를 시도하지 않는다. initializer는 IF를 `IL_4807`, IF_THEN을 `IL_491b`에서 Add한다.

이전 조사에서 남긴 framework enumeration 의문을 이번에 좁혔다. `MathcadPrime.runtimeconfig.json` SHA `c71152bf25e40d647b2440c5b39be157a3d356106be9d5b678ab97bb87b4e782`는 `tfm=net8.0`, `Microsoft.NETCore.App`/`Microsoft.WindowsDesktop.App` version `8.0.0`을 선언한다. 현재 설치 filesystem의 .NET 8 shared runtime은 8.0.29이며 위 B를 정적으로 조사했다.

1. E DefaultDescriptorTable constructor `0x06000bbf`는 fresh Dictionary 기반 constructor `0x06000bcb` 후 `f`를 실행한다. 이어 `c`는 별도 FunctionOperatorsList, `b`는 별도 IDLabelMapDictionary를 만든다. `f`와 하위 초기화에서 IF/IF_THEN을 remove/reinsert하는 경로는 확인되지 않았다. E의 Dictionary mutation cross-reference도 해당 descriptor table의 Remove/Clear/overwrite를 보이지 않는다.
2. B Add `0x060084df`는 TryInsert를 호출한다. `0x060084ec IL_015e..01c0`는 free slot이 없으면 index=`_count`를 사용하고 `_count`를 증가시킨다. Resize `0x060084ef IL_0000..0016`는 기존 entries를 Array.Copy로 index 순서를 유지한다. bucket chain을 다시 연결해도 entries 순서는 바뀌지 않는다.
3. B Dictionary Enumerator constructor `0x0600850b IL_0013..0015`는 `_index=0`; MoveNext `0x0600850c IL_001a..005d`는 `_entries[_index++]`의 살아 있는 항목을 반환한다. 따라서 해당 fresh Add-only table은 등록 순서로 열거된다.

**결론: 이 설치의 선언된 기본 framework 구성과 기본 descriptor table에서는 Ctrl+J `if`가 IF_THEN보다 IF를 먼저 선택한다.** 설치 runtimeconfig와 정적 파일은 실제 시작 시 외부 runtime override, 다른 host, 사후 custom table 변이를 관찰한 증거가 아니므로 그 환경까지 무조건 보장하지 않는다. 이 제한을 이유로 기본 계약의 IF 경로를 다시 임의 추정으로 취급할 필요는 없다. Ctrl+J 일반 populated-caret 복구 전체와 IF subelement 강제 변환까지 증명한 것도 아니다.

## Blank branch 삽입과 원자적 편집의 범위

E `bg.a` `0x060005c5`가 pair subelement의 핵심 경로다. 다음 조건은 단순히 가장 가까운 IF에 append하는 알고리즘과 다르다.

- 대상 자신이 declared parent IF이고 caret이 오른쪽이며 final else가 없으면 마지막 경계에 삽입한다(`IL_001d..0059`).
- placeholder가 Program family parent 안에 있고 index>0이면 앞 sibling의 incomplete pair를 찾을 수 있다(`0072..00d1`). enum `OperatorFamily.Program`은 E field `0x040006a8`, Constant type I4 raw `03 00 00 00`=3으로 직접 대조했다. 이전 중간 계약의 Family=3 불확실성은 해소됐다.
- 그 외에는 declared parent pair의 nearest ancestor를 찾는다(`00d7..00ed`); 없으면 Empty다.
- 이미 final else가 있으면 또 else를 넣거나 그 마지막 body 내부에서 새 branch를 만드는 경우를 거부한다(`0131..0169`). else는 pair의 마지막 삽입 경계 이전에 들어갈 수 없다(`027d..02a7`).
- ordinary/alternate middle 종류는 기존 pair와 맞아야 한다(`02a8..02f3`). operator-centered repair `a2.a` `0x06000540 IL_02bb..033a`는 위치에 따라 전체 alternate flag를 바꿀 수 있다. 따라서 개별 branch 종류만 바꾸는 모델은 부정확하다.
- 성공 경로는 필요한 placeholder들을 생성하고, 필요 시 기존 child container를 split하여 뒤 material을 보존하고, 앞 placeholder statement를 소비하며, repair 후 새 child로 이동한다(`02f4..03eb`). 구체적인 caret은 BODY repair와 결합된다.

원본 `a2.a`에는 previous sibling pair merge, 현재 pair split, Program parent 생성도 있다. 이번 bounded 구현이 IF Right, incomplete IF 다음 blank, middle operator toggle만 지원한다면 그것을 지원 범위로 기록해야 한다. 모든 populated target·중첩 container 경계를 동등하게 처리했다고 확장하지 않는다. 실패 시 문서/history/redo 불변과 성공 시 한 Undo로 tree·caret·selection·scope 복원은 제품 테스트로 확인해야 하는 계약이며 이 정적 조사에서 원본 Undo 시스템을 새로 실행해 관찰한 것이 아니다.

## 원본은 visible `then`을 표시하지 않고 body를 아래에 쌓는다

C `VisualFactory.CreateIfNode` `0x060006a4`는 문자열 `if`, `else if`, `else`를 VisualPairNaryNode에 전달한다. overload `0x060006a5`는 alternate flag에 따라 middle을 `else if`/`also if`로 고른다. **여기서 `then`은 표시 문자열이 아니다.** native XML의 `<then>`을 화면에 영어 단어로 출력할 근거가 없다.

C `VisualPairNaryNode.PrepareVisualChildren` `0x06000b9e`는 child count에 따라 first/middle/final symbol을 만든다. `MeasureOverride` `0x06000ba4`와 `ArrangeOverride` `0x06000ba5`는 실제 배치를 결정한다.

- header operator와 test는 text baseline을 맞춘다(`Measure IL_02fb..032c`). test x는 operator width + 4(`0331..0366`).
- test height만큼 y를 증가시킨 뒤 body는 x=8, 새로운 y에 놓는다(`036b..039e`). body height를 더한 뒤 다음 branch로 간다(`03a3..03c7`).
- final else는 token header 자체의 height를 더한 뒤 그 아래 body를 x=8에 놓는다(`028e..02f4`). 즉 else+body도 한 줄로 납작하게 붙지 않는다.
- descriptor IF/IF_THEN의 ChildrenRows=[0,1], NavigatablePartRows=[0]도 header/body 행 구분과 일치한다(E `0x06000bbb IL_4788..47a1`, `489c..48b5`).
- C pair constructor `0x06000b99`는 오른쪽 minor vertical fence를 만들고 opacity=0.3으로 둔다. Measure의 fence x는 content width, y=0이다(`03d1..03f4`). 이는 body 왼쪽 프로그램 바와 별개다.

BODY도 왼쪽 프로그램 바를 가진다. C CreateProgramNode `0x060006a0`는 true, CreateBodyNode `0x060006a1`는 false를 **같은 VisualProgramNode constructor** `0x06000bdc`에 전달한다. 이 bool은 `_hasMinorFence`다. constructor는 왼쪽 `CreateDoubleVerticalLine`을 항상 만들고(`IL_000d..0024`), true일 때에만 자기 오른쪽 minor fence를 만든다(`002a..0062`). operands panel은 한 column이다. Measure `0x06000be4`는 왼쪽 fence content height를 panel height+8로 설정하고 전체 width에 fence width를 더한다. 따라서 BODY를 쓸 때 왼쪽 바를 임의로 지우거나 두 번째 right fence를 추가하면 안 된다.

`CreateDoubleVerticalLine`의 이름도 두께 옵션으로 추정하지 않았다. C `0x06000574`는 char[4] 정적 데이터를 VerticalFence(char[])에 전달한다. FieldRVA `0x000d7128`, field `E5AAD264489C303BAE0C94BF98DD5B3941579522230BAECA9DDDE7C3FE812B9D`의 8 bytes는 `16 20 16 20 16 20 16 20`, 즉 **U+2016 `‖` glyph 네 개**다. Single line `0x06000573`의 RVA `0x000d7118` bytes는 `7c 00 7c 00 7c 00 7c 00`, U+007C `|` 네 개다. 양자는 다른 glyph 구성이다. 실제 gap, 조합, font metrics, DPI stroke까지 같은지는 별도 검증이 필요하다. 임의 단일선 stroke를 그리면서 원본 double glyph와 정확히 같다고 주장하지 않는다. 수치 4/8은 이 WPF 코드의 좌표 값이며 재구성 renderer의 임의 font scale 규칙까지 증명하지 않는다.

## 선택한 메서드 원시 IL SHA-256

| 어셈블리·token | 역할 | IL SHA-256 |
| --- | --- | --- |
| E `0x0600119d` | IF writer | `980e13f854710baf3f764546c6cfb5eb39dd4f7cf0a34f1ae7f0412aba260c53` |
| E `0x060011a4` | test/then writer | `5163bb5899d4b08a9dffe697794f662832dce8c4f3dc8481dbcab73bd6380624` |
| E `0x06001174` | XML name closure 생성 | `cd5889aa62cbdfb2d3f4e103dc431adbe4c593a2d0df47cf8e0fd2c4c8564c62` |
| E `0x060011ac` | closure dispatch | `1a436eb3a59c053a4b8bddfdfd5829f64215496286c0dd9a2ddc652608832a05` |
| E `0x06001175` | 이름 지정 element writer | `f3ce9245d37618561f4ab32114602d9ad3d8ad6b6a3c27700bc304a73e002124` |
| E `0x0600116d` | TransformChildNodes | `39887235cb62445c11561e10b35bc8587871cc4a0c615654fbeb5d01510a299d` |
| E `0x06001270` | IF reader | `c1fdc956363e27b4b439f59f36a49276678d3cfe4de11f2ee324d5fcf9f7b7e9` |
| E `0x06001273` | test/then reader | `6e02a90da800e42592a15d41ea8b3de5da89acfb08958a982e722cfec2d6afd3` |
| E `0x0600058d` | ReparentChild | `67661c915a1eb4a0e9ca769fb13e3d0b43ce6270d2a7bef2f02080bafe84433a` |
| E `0x06000590` | ReplaceChildDescriptor | `a069333bd64356e0889066cb497fb778749f7400bfb18b5490381059ccf05a42` |
| E `0x0600059e` | Ctrl+J | `dbaea23c9588a86b123e3b7d67e5b4c86467ce28b38ef1873559dd97a2bb9f0c` |
| E `0x060005c5` | branch insertion | `e38359a22e9d90dc1436596608acb0d2916546a63280bd8107a5528102ba5957` |
| E `0x06000bbf` | default table constructor | `47ad5171693a4e255e2c08b52dd64445c29e18d263f0fe9725168ebfaa71b9dc` |
| B `0x060084ec` | TryInsert | `f65cd2fbce70c702ab3368625bcde859cc2ab5054fe97fba86c20a7a42db0344` |
| B `0x060084ef` | Resize | `38c4eed83f31d2904d7df062e5b6444642f02c660c7becffd2c22e26c59cc8bd` |
| B `0x0600850b` | Enumerator constructor | `1ad89b2d38160574c4b5a8dd296f4cfe0debc864234437b6ee27c8f2a71dda6d` |
| B `0x0600850c` | Enumerator MoveNext | `d9c063cd6fd9277fd18e4c3468ff85f2666d64e878e75f51c0415e6c13583446` |
| C `0x060006a4` | IF tokens | `9297ea2c5b547638576bc2f2ed14eacd7bb592f987e83dd6a0369d9ce57370b5` |
| C `0x060006a5` | alternate IF tokens | `134cb02051c6274bfd6348e33c9b677f4e03090b8be44e63e90f84a9b01e370d` |
| C `0x06000b99` | pair fence constructor | `4448bef034e7f968a6d88e4b544849aa6919c745f18f322d62acda1c60deb706` |
| C `0x06000b9e` | branch symbol array | `98a7ff8704147c403a8122b7197edba4588c6774f0abf25f09a9c6f12613f0ba` |
| C `0x06000ba4` | pair MeasureOverride | `21726623d543cb5b931d4e1072033387c4beeb2f6db441835d0fc666592114c5` |
| C `0x06000ba5` | pair ArrangeOverride | `fadd44801d1b4ec9fff1476f084b72aece4bed7f170316e7e19dc9468ff0533d` |
| C `0x060006a1` | BODY visual | `b30748104d34267643c90deb6812782f6530d247437e57d77e1f1d237f2e10e4` |
| C `0x06000bdc` | Program/BODY constructor | `314dd228340f5399195b45b73b0f7dd6d206eecf3248c79f67089612410c3fcb` |
| C `0x06000be4` | Program/BODY MeasureOverride | `6941f96812e211c2f35a99ba4259211a9e05b8b57d40f6116ea2e4f8f2f531bc` |
| C `0x06000573` | single glyph fence | `da97b1412dcdffec3f34cdee39affd105c27327a9826c107a4018c0d551e6236` |
| C `0x06000574` | double glyph fence | `3f0261c281c5e3eb86758a0a1790004485653c7de6f617a1289a162095d55e6a` |

남은 한계: 원본 계산 엔진의 raw mixed-branch 처리, generic populated-caret split/merge 전체, 모든 nested BODY 복구 상태, 원본 font/DPI glyph 조판 동등성, TRY native/evaluation 계약은 이 문서로 완료되지 않는다. 새 연산자나 원본에 없는 계산 명령을 추가할 근거도 아니다.
