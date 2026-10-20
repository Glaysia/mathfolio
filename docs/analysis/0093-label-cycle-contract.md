# Ctrl+Q 라벨 순환 계약 — 2026-10-19

[FEATURES EA-016](../FEATURES.md)의 원본 입력 계약 조사다. 제품 코드는 수정하지 않았다. 설치 Prime 12.0.0.1 도움말과 기존 REA managed-artifact 증거를 재사용하고, 선택한 CIL 본문을 설치 파일에서 읽어 REA의 raw IL SHA-256과 일치하는지 검사했다. GUI·스크린샷·원본 앱·CLR·COM·어셈블리 로딩/실행을 사용하지 않았다. 아래는 정적 관찰이며 런타임 적합성 완료 판정이 아니다.

## 근거의 신원

| 기호 | 설치 파일·REA Evidence | SHA-256·MVID |
| --- | --- | --- |
| E | `Ptc.EquationEditor.dll`; `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf` | SHA `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`; MVID `d8627dca-3258-470c-b4d1-c00bbe2f2441` |
| C | `Ptc.Controls.EquationControl.dll`; `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349` | SHA `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`; MVID `cf1d7cb5-c73f-4618-8e6b-b83959d13b98` |
| K | `Ptc.Controls.Core.dll`; `ev_f208e991f9e1e1e33663525464eb86305e6bee7352187fed8011d74326426e64` | SHA `654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977`; MVID `b5e8e4fd-c134-4713-b1b1-eea8cddcdd9f` |

각 파일은 `C:/Program Files/PTC/Mathcad Prime 12.0.0.1/` 아래에 있다. 원시 REA는 `artifacts/rea/editor/{equation,control}-members.json`, `artifacts/rea/document-deep/controls-core-members.json`에 있다. 보조 조회기 `artifacts/rea/feedback/label-cycle-inspect.py`는 해당 파일 SHA와 선택 메서드의 CIL SHA를 다시 확인한다. 기존 `context-labels-deep/` 자료도 조회했다. 원본 본문은 추적 문서나 제품 코드에 복제하지 않는다.

설치 도움말 `Help/en/PTC_Mathcad_Help/about_labels.html`, SHA `355b735fe41d46eb702a385666ea002467230b87fb95a52164d81f10d2be9183`는 Ctrl+Q의 라벨 순환, 자동·변수·단위·상수·함수·시스템·키워드 종류와 contextual 표시를 설명한다. `to_apply_a_label.html`, SHA `ba5d66655736b48535f28128275e590dec9ee631ecdddd2cf5f9773ac9573173`는 선택한 수학 요소에 라벨을 적용하고 Automatic으로 자동 라벨링을 복원하는 명령을 설명한다. **도움말 나열 순서를 순환 순서로 간주하지 않고 아래 바이너리 분기로 별도 확인했다.**

## 실제 순환과 시작점

K `Ptc.Controls.Core.IdLabels`, TypeDef `0x0200004e`의 Constant 테이블은 다음과 같다. 값은 메타데이터를 직접 파싱한 결과이며 CLR reflection을 사용하지 않았다.

| 필드 | field token | 값 |
| --- | --- | ---: |
| Variable | `0x04000121` | 0 |
| Unit | `0x04000122` | 1 |
| Constant | `0x04000123` | 2 |
| Function | `0x04000124` | 3 |
| System | `0x04000125` | 4 |
| Keyword | `0x04000126` | 5 |
| None | `0x04000127` | 6 |

E `EquationModel.ToggleIdLabel`, `0x060001b0`, IL SHA `20bf985af5afe58be522148242824f5129488d9cc467482d4c6f612183898d9f`는 현재 mode의 허용 검사를 통과한 뒤 enum 목록과 **명시 IdLabel**을 읽는다.

- `IL_0009..0018`: `Enum.GetValues(IdLabels)`.
- `IL_0032..0047`: `HasNoLabel`이면 enum 첫 값으로 `SetIdLabel`.
- `IL_004d..0082`: 현재 값이 enum 마지막이면 `UnsetIdLabel`.
- `IL_0083..0094`: 나머지는 현재 `IdLabel.Value + 1`로 `SetIdLabel`.

E `IdentifierNodeInfo.get_HasNoLabel`, `0x06001081`, IL SHA `df05d28a8545836de85fc2fdd6155341e59d4870d0dd4070f004916411bfd979`는 K `IdLabelsExtensions.IsNullOrNone`, `0x060002af`, IL SHA `330c7b3ebafa6d5fc764b23ebfe6468a89de8396fff3fd3e451e39eff911e3bd`를 호출한다. 이 함수는 nullable 값 없음 **또는 값 6**이면 true다.

따라서 실제 순환은 **Variable → Unit → Constant → Function → System → Keyword → None/Automatic → Variable**다. 처음 명시 라벨이 없으면 resolved/display 라벨이 Unit이더라도 Variable부터 시작한다. Keyword의 다음 단계는 `SetIdLabel(6)`이다. 이 None은 다음 키에서 `HasNoLabel` 분기로 들어가므로 위의 마지막 값/Unset 분기는 정상 enum 값 6에 대해서는 사용되지 않는다. None을 nullable null로 바꾸는 정규화도 이 경로에는 없다.

## 적용 대상·선택·아래첨자

C `EquationControl.OnToggleIdLabel`, `0x0600017b`는 view의 `FireToggleIdLabel`로 전달한다. 명령 등록부는 `EquationCommands.ToggleIdLabel`과 `CanPerformIdLabelOperations`를 연결한다. C `CanPerformIdLabelOperations`, `0x0600017e`는 view를 거쳐 E `EquationModel.CanPerformIdLabelOperations`, `0x0600011d`로 연결되는 허용 검사를 사용한다. E public toggle 자체도 같은 mode 검사를 다시 한다.

E 기본 mode 검사 `b.a0`, `0x06000208`, IL SHA `3907e8f1290d03b171f6132d6070753ca2d3130bb66bd1837c226039e414050a`의 조건은 다음뿐이다.

1. 현재 navigator의 Current가 존재한다.
2. Current의 NodeInfo가 `IdentifierNodeInfo`다.
3. `ResolvedLabelOverridesIdLabel(Current)`가 false다.

E `EquationEditModeEnum`, TypeDef `0x0200008b`의 Constant 값은 Normal=0, Scope=1, Typeover=2다. mode 구현의 `e()` 반환값은 각각 `l/0x0600006f`=0, `m/0x06000277`=1, `n/0x060002a7`=2다. 난독화된 짧은 이름만으로 모드를 추측하지 않았다.

- Normal은 위 기본 검사를 사용한다.
- Scope의 `m.a0`, `0x0600027a`, IL SHA `af5a02795d65ee70f4ca6181d0d7ce28332a54045431588f6cda180cac008398`는 선택 segment의 left node와 right node가 같은 경우에만 기본 검사를 호출한다. E `ak.t/s`, `0x060003d1/3d2`가 각각 `LinearSegment.LeftNode/RightNode`임을 확인했다. 여러 노드를 가로지르는 선택에는 허용하지 않는다.
- Typeover의 `n.a0`, `0x060002ac`는 false를 반환한다. **Typeover와 read-only는 같은 개념이 아니다.**

변경 구현 `b.as(IdLabels)`, `0x06000259`, IL SHA `0b6e6e8b0b41aaea8a1241ac514a78a66423d30afe9c7c1f1ea1c4d9c5c83c3f`는 현재 identifier의 IdLabel만 바꾼다. FormattedString 값, 문자 선택 범위, literal subscript 내용을 다시 쓰는 코드가 없다. 따라서 라벨은 선택한 문자 조각의 글꼴 변경이 아니라 식별자 전체의 속성이다. 아래첨자만 선택한 모든 navigator 상태의 동작은 실행 검증하지 않았으므로 suffix의 세부 진입/선택 규칙까지 완료로 세지 않는다.

## 구조가 명시 라벨을 덮어쓰는 guard의 정확한 범위

E `EquationModel.ResolvedLabelOverridesIdLabel`, `0x06000143`, IL SHA `24bc13e8b1c7ff19428ba0dc14f719d134cbaffb16e4b3ba7b04da824c2518e7`의 반환식은 다음과 같다.

- 인자가 null이면 false.
- `OverrideIdLabelByResolvedForDefinitionLhs`가 false이거나 현재 descriptor가 `IDENTIFIER`가 아니면 false.
- 기준 노드는 EquationRoot가 `DEFINITION`이면 그 child 0, 그 외에는 EquationRoot 자체다.
- 현재 노드가 그 기준 노드와 같으면 true.
- 기준 노드가 `MATRIX_INDEX`이면 현재 노드가 그 child 0과 같을 때 true.
- 나머지는 false.

flag getter/setter `0x06000141/142`는 field `0x04000068`을 읽고 쓴다. setter는 bool 저장만 한다. E 안의 쓰기 참조는 setter뿐이며 생성자에서 true로 설정하지 않는다. C `0x06000036/37`은 이 속성을 model에 전달한다. 이 조사에서 호스트마다 어떤 경우에 flag를 true로 설정하는지는 확정하지 않았다.

이 guard 안에는 함수명·프로그램 local 정의·적분/합의 bound variable 자손을 재귀적으로 찾거나 모두 차단하는 분기가 없다. `LOCAL_DEFINITION`, `GLOBAL_DEFINITION`, `FUNCTION`을 여기의 `DEFINITION`과 임의로 같은 것으로 처리해서는 안 된다. 다른 structural auto-label repair가 존재한다는 사실도 Ctrl+Q 금지의 직접 근거가 아니다. 일반 contextual Unit을 이유로 금지하는 조건 역시 없다.

원본의 `get_IsReadonly`, `0x0600010a`는 restricted read-only, Scope/Typeover와 노드의 read-only를 별도로 계산하지만 위 기본 라벨 guard에서는 이를 직접 호출하지 않는다. 상위 문서 잠금/명령 활성화와 계산 결과의 비편집 상태 전체를 이번 조사만으로 설명하지 않는다. 현재 앱 자체의 비편집 원본 영역을 수정하지 않는 기존 보호를 없애야 한다는 뜻도 아니다.

## None·contextual과 직렬화

E XML reader `XmlNodeToMathNodeTransform.a`, `0x0600125e`, IL SHA `2929e0d2ec78839dc0a44615a5c526f39b1fa223bc9895602831bdfe076b9cb3`는 `labels`의 첫 토큰(또는 구형 `label`)을 변환한다.

- `label-is-contextual=true`이면 변환된 라벨을 **ResolvedIdLabel**에만 넣는다.
- 그렇지 않으면 nullable **IdLabel**에 넣는다.

따라서 `labels="UNIT" label-is-contextual="true"`로 읽은 일반 식별자의 명시 IdLabel은 null이며 Ctrl+Q의 첫 단계는 Variable이다.

E XML writer `MathNodeToXmlTransform.h`, `0x06001176`, IL SHA `e4e82c7c8863695980620eda29a334866387c286f2d9ca35822e26822b2cf6ed`는 다음을 구분한다.

| 내부 상태 | XML 출력 |
| --- | --- |
| IdLabel=명시 0..5 | `labels="VARIABLE/UNIT/CONSTANT/FUNCTION/SYSTEM/KEYWORD"`; contextual flag를 추가하지 않음 |
| IdLabel=명시 None(6) | `labels="*"`; contextual flag를 추가하지 않음 |
| IdLabel=null, ResolvedIdLabel≠None | `labels="해결된 라벨" label-is-contextual="true"` |
| IdLabel=null, ResolvedIdLabel=None | labels/contextual을 추가하지 않음 |

K `IdLabelToStringConverter.Convert`, `0x060002b2`, IL SHA `de626b981a42114664023bc76521215f579e4f1acc58954c0b96d1528a30c292`는 None에 옵션 true가 주어지면 `*`를 반환한다. 위 writer가 true를 넘기는 것을 확인했다. K 역변환 `0x060002b4`는 옵션 true에서 `*` 또는 `NONE`을 값 6으로 읽는다.

**XML 보존과 계산상 라벨 선택은 구분해야 한다.** E `GetLabelRLSuffix`, `0x060010f8`는 K `IdLabelsExtensions.GetRl`, `0x060002b0`, IL SHA `e25981907a030e08c0c31968fe17b83fc47b6db4abde7685cf9eb5c89c176f6b`를 사용한다. GetRl은 null과 None을 모두 `IsNullOrNone`으로 처리하여 같은 fallback 인자를 선택하고, 명시 0..5에만 라벨 문자열을 붙인다. display getter `0x06001084` 역시 HasNoLabel일 때 ResolvedIdLabel을 사용한다. 따라서 `*`를 별개의 사용자 심볼 라벨로 취급하면 이 계약을 재현하지 못한다. 다만 전체 엔진의 wildcard/type-inference 알고리즘을 여기서 복구한 것은 아니다.

## Undo 경계

E `SetIdLabel`, `0x060001ad`, IL SHA `6f845a81f076d9eee8f457931dea1ddedd234d550148f375e43faf7de626077b`는 한 번의 `Set ID Label` 명령 객체(`e` 생성 `0x06000b63`, 실행 `0x06000b6b`)를 만들고 mode의 변경 함수를 호출한다. 변경 전후 editor 상태 snapshot을 보존하며 Undo/Redo `0x06000b68/69`가 각각 저장 상태와 부수 CompoundCommand를 복원한다. 입력 문자 삽입 함수로 재구성하거나 현재 이름을 다시 타이핑하는 경로가 아니다.

더 낮은 `IdentifierNodeInfo.set_IdLabel`, `0x06001080`, IL SHA `ac0b0c3626d707d08db86de5d5a69b8e7377ee46aa14bf1cda4d3dc840804dc6`는 nullable 새 값과 이전 값을 캡처하고, Undo tree scope가 있으면 `Set ID label` 명령에 넣는다. closure `0x0600108a`가 nullable 새 값을 field에 그대로 쓰고 `0x0600108b`가 이전 값을 그대로 복원한다. None을 null로 바꾸지 않으며 FormattedString/literal suffix도 바꾸지 않는다.

구현에 옮길 때 한 Ctrl+Q를 한 원자적 편집으로 기록하고 기존 label/metadata를 Undo로 복원하는 것은 이 관찰과 맞는다. 상위 연속 명령 간 history 병합 정책 전체와 결과 캐시 복원의 모든 시점은 별도 확인 대상이다. 단축키가 동작했다는 이유로 계산 후 contextual label·문서 저장·Undo까지 자동으로 검증됐다고 세지 않는다.

## 후속 검증 범위

독립 구현에서는 최소한 일곱 단계와 재순환, null/contextual Unit에서 Variable 시작, `*` MathML 보존과 자동 라벨 추론, 명시 Unit과 contextual Unit의 차이, 여러 노드 선택 거절, 이름/literal subscript/UTF-16 문자 선택 보존, Undo/Redo를 검사해야 한다. 현재 원본의 read-only 상위 명령 라우팅, 아래첨자 내부의 모든 Scope 상태, Override flag를 true로 만드는 호스트, 구조별 auto-label repair와 강제 override의 모든 상호작용은 미확인이다. 이 조사만으로 EA-016을 구현됨으로 바꾸지 않는다.
