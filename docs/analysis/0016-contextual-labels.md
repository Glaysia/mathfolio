# Prime 12의 명시 라벨과 문맥 라벨

2026-10-15. GUI, 스크린샷 및 원본 실행 없이 기존 REA metadata와 원본 raw CIL에 해시를 대조한 supplemental decode를 조사했다. 핵심 질문은 저장된 `labels="UNIT"`가 언제 사용자의 명시적 선택이고, 언제 이전 계산이 알려 준 표시 정보인가이다.

## 원본의 두 상태

대상 `Ptc.EquationEditor.dll` SHA-256은 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID는 `d8627dca-3258-470c-b4d1-c00bbe2f2441`이다. REA evidence는 `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`이며, ignored supplemental decode는 `artifacts/rea/editor/equation-il.json`이다. 이는 정적 호출·분기 증거이며 실행 관찰이 아니다.

| 원본 경로 | 정적 관찰 | 복원에 필요한 구별 |
|---|---|---|
| `XmlNodeToMathNodeTransform.a`, `0x0600125e`, IL_00b1–00e4 | 라벨이 있고 contextual 플래그가 없거나 false이면 `IdLabel`을 설정한다. true이면 `ResolvedIdLabel`만 설정한다. | contextual 라벨을 명시적인 타입으로 고정하면 안 된다. |
| `MathNodeToXmlTransform.h`, `0x06001176`, IL_002c–00bf | 명시적 `IdLabel`은 `labels`만 기록한다. 명시 라벨이 없고 resolved 라벨이 있으면 `labels`와 `label-is-contextual=true`를 기록한다. | 저장과 재계산 사이에 표시 캐시와 사용자 선택을 보존해야 한다. |
| `MathNodeToSExpressionTransform.a`, `0x06001164`, IL_000f–0028 | `ShowResolvedIdLabels`가 false이고 `HasNoLabel`이면 표시용 라벨이 있어도 원래 식을 반환한다. | 일반 엔진 입력은 저장된 표시 캐시를 무조건 강제하지 않는다. |
| `EquationModel.a`, `0x06000145`, IL_00d2–010f | 결과의 resolved identifier 정보를 읽고 해당 식 노드의 라벨 상태를 갱신하는 경로가 있다. | 라벨은 편집 후 계산에서 다시 받은 피드백일 수 있다. |

각 메서드 raw body SHA-256은 순서대로 `2929e0d2ec78839dc0a44615a5c526f39b1fa223bc9895602831bdfe076b9cb3`, `e4e82c7c8863695980620eda29a334866387c286f2d9ca35822e26822b2cf6ed`, `510fadbd43bc2dc916ed1d9ee35866b2d6e3dac1f0f069748fbd249c210c6001`, `1c54b5c6d438b0f26d9d3bd3d1d5fe0900723ef21a193256ff80f8f2f7826f89`이다.

UX 관점의 해석은 다음과 같다. `m := 7`을 쓴 뒤 이전에 단위로 표시되었던 자동 라벨의 `m`을 계산할 때, 이전 표시를 영구 타입으로 오인하면 문서가 사용자의 현재 정의를 따라가지 않는다. 명시적으로 Unit을 선택한 `m`은 별개의 사용자 선택이므로 그대로 존중해야 한다. 원본이 저장 포맷에 두 상태를 구별한다는 점이 직접 증거이며, 이 시나리오 전체의 원본 실행을 관찰했다는 뜻은 아니다.

## 구조가 확정하는 라벨

`DefaultDescriptorTable.f` (`0x06000bbb`, raw body SHA-256 `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a`)의 제약을 조사했다.

- Define/GlobalDefine의 child 0에는 먼저 `AutoLabelIdChild`가 적용되고, 이어 `AutoLabelIdDescendant(VARIABLE=0, child=0)`가 적용된다(IL_1a2b–1a57, IL_1bc2–1bee). 함수 선언과 매개변수도 이 하위 트리에 포함된다. 사용자 함수 이름을 무조건 FUNCTION으로 만들지 않는다.
- Evaluation의 child 1은 RSCALE이며 `AutoLabelIdDescendant(UNIT=1, child=1)`가 적용된다(IL_20ab–211c). 표시 단위 입력창은 일반 수식 입력과 문맥이 다르다.
- Matrix index/row/column은 child 1에 재귀 자동 라벨을 금지한다(IL_0e29–0e35, IL_0fd5–0fe1, IL_10c5–10d1). 인덱스 식 안의 변수에 바깥 표시 단위 문맥을 전파하면 안 된다.
- `AutoLabelIdDescendants`, `0x06000ca2`는 이미 `IdLabel`이 있는 노드를 변경하지 않는다. 금지된 child는 `ForceUnlabeledIdDescendantsToHaveLabelNone`, `0x06000ca4`로 보내 명시 라벨 없는 노드만 None=6으로 만든다. None은 UNIT 또는 VARIABLE로 강제한다는 뜻이 아니다.

`AutoLabelID` (`0x06000ca8`)는 명시 라벨이 없는 **직접 identifier**만 reserved-name map으로 검사한다. `DefaultDescriptorTable.b` (`0x06000bc1`, raw SHA-256 `8621c3a10d7ce6f567acf3a1fc29ea47b2ae8142d90a506fd97e26d89dcb5f04`)의 map은 `ORIGIN`, `TOL`, `CTOL`, `PRNCOLWIDTH`, `PRNPRECISION`, `CWD`, `Seed`, `in0`부터 `in9`까지를 SYSTEM=4로 분류한다. 이 사실은 CWD를 숫자로 만들거나 Seed에 숫자 기본값을 주는 근거가 아니다. 값과 부작용의 경계는 [SYSTEM 기본값 조사](0013-system-defaults.md)를 따른다.

## 적용한 독립 구현

`editor_bridge`의 저장 경로는 contextual 플래그와 literal subscript를 그대로 roundtrip한다. 사용자가 수식을 다시 계산할 때만 생성하는 calculation XML에서 다음을 적용한다.

1. contextual이 없거나 false인 명시 라벨은 유지한다.
2. contextual=true의 이전 표시 라벨을 강제하지 않고, 확정된 선언/매개변수/표시 단위 구조 제약을 적용한다.
3. 나머지는 기존의 제한된 visible-definition/catalog 조회 정책을 사용한다. 문자열 자체와 literal subscript를 합친 identity로 조회하여 `Z`의 literal subscript `in`과 평문 `Z_in`이 충돌하지 않게 한다.
4. 원본 문서의 저장 XML은 바꾸지 않는다. 생성한 calculation XML에만 결정한 라벨을 명시하여 bounded evaluator로 전달한다.

이 마지막 조회 정책의 전체 우선순위가 Prime과 동일하다는 결론은 내리지 않았다. 특히 여러 namespace의 동명 정의, wildcard, symbolic label, index 안의 모든 free ID, engine의 자유 식별자 해석은 추가 분석 대상이다. index 안에서는 구조적 UNIT 상속만 막고 기존 제한 조회로 넘긴다. 직접 imported MathML의 unresolved contextual ID는 전체 resolver가 마련되기 전에는 원본 캐시를 강제하여 틀린 값을 반환하기보다 미지원으로 드러내는 것이 현재 경계다.

`tests/contextual_labels.rs`는 캐시 Unit/명시 Unit 쌍, 자동 라벨의 미정의 이름, 표시 단위와 동명 변수, 함수 매개변수, 인덱스 상속 차단, reserved SYSTEM 선언, literal subscript identity를 검증한다. 7개 신규 headless 테스트와 기존 structured 9개가 통과했다. 전체 compat 테스트도 통과했다. 원본 UI 및 원본 엔진과의 runtime parity 검증은 하지 않았다.
