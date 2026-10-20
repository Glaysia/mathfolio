# 행·열 대입의 원본 저작·저장 계약

2026-10-19. 기존의 행·열 **추출** 연산자를 정의의 왼쪽에 놓는 기능을 별도로 조사했다. GUI, 스크린샷, 원본 앱, CLR, COM을 실행하지 않았다. 설치 DLL의 REA metadata/raw CIL과 설치 도움말을 읽었고, 독립 Rust 편집·XML 경로를 검사했다. 수치 대입의 shape, 확장, 단위, 취소 계약은 별도 계산 조사 범위다.

## 원본에서 허용하는 왼쪽 식

`Ptc.EquationEditor.dll` SHA-256은 `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, MVID는 `d8627dca-3258-470c-b4d1-c00bbe2f2441`이다. 기존 retained evidence `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`의 raw method 위치와 SHA를 재사용하고 파일 바이트를 다시 대조했다. 이번 REA 선택 조회 parent evidence `ev_1f215f1e6931078c354d7813146ef97dfb76328f5125a3e3662173fa4c29feaf`는 `DefaultDescriptorTable` 및 `Tree.Transform.MathNodeToXmlTransform`을 대상으로 한다. 다른 namespace를 지정한 앞선 조회에서 writer type이 unmatched였던 결과는 writer의 부재 증거로 쓰지 않았다.

`DefaultDescriptorTable.f`, token `0x06000bbb`, raw IL SHA `90ff4afc15ec4d75068d5d2c0eab08be91548ff38974f8de5b54e02a25a6376a`는 다음 whitelist를 직접 설정한다.

| descriptor | child 0의 RestrictChild 설정 | 확인한 행·열 항목 | parent 계약 |
| --- | --- | --- | --- |
| DEFINITION (`define`) | IL 6594–6672 | MATRIX_ROW 6653, MATRIX_COLUMN 6661 | ROOT, IL 6677–6694 |
| GLOBAL_DEFINITION (`globalDefine`) | IL 7001–7079 | MATRIX_ROW 7060, MATRIX_COLUMN 7068 | ROOT, IL 7084–7101 |
| LOCAL_ASSIGNMENT (`localDefine`) | IL 7446–7530 | MATRIX_ROW 7505, MATRIX_COLUMN 7513 | program 명령의 별도 context |

세 목록 모두 MATRIX_INDEX도 포함한다. 따라서 행·열 LHS 지원을 임의의 편의 기능으로 추가한 것이 아니다. LOCAL_ASSIGNMENT의 RestrictChild overload는 `PullOutAdjacentDescendantFromChild` 변환을 함께 제공하지만 이것을 독립 편집기의 모든 navigator 동작을 복원했다는 증거로 확대하지 않는다.

각 정의는 LHS subtree에 `AutoLabelIdDescendant`도 설정한다(지역 정의 6723–6743, 전역 7130–7150, 프로그램 7535–7555). 행·열 descriptor는 추출 index에 대한 auto-label 상속을 차단한다. 따라서 이름 `m`을 행렬의 대입 대상으로 쓰면 기본 VARIABLE이고, 그 행·열 index의 `ORIGIN`이나 단위 `s`를 전부 VARIABLE로 강제하면 안 된다. 명시적으로 지정한 label과 literal suffix는 별도로 보존한다.

## 저장 표현

행·열 operator 자체의 계약은 [0069](0069-structural-array-operators-contract.md)와 같다. descriptor 이름은 MATRIX_ROW/MATRIX_COLUMN, XMLFormat은 `matrow`/`matcol`이다. `row`/`col`이라는 새 native operator나 scalar `indexer`로 치환하지 않는다.

`MathNodeToXmlTransform.a` `0x06001167` raw IL SHA `3930351561a894c121a700a2d9fa21cb2d5c8966dbba30f0550897f7c2136e89`의 dispatch, `TransformDefaultNode` `0x06001173` SHA `2749dcc2ffbbef2b8312c385799049d9b52c22f2f2a32b257be339145e6251b2`, `TransformOperatorNode` `0x06001177` SHA `edd18405d02dfd8f5cc817f0fe06287202b091b678d03d13bd1cd2cb078f7ba7`를 재검증했다. 정의의 child를 저장할 때도 operator writer를 통과한다. 예를 들어 지역 열 대입은 다음 구조다.

```xml
<ml:define>
  <ml:apply><ml:matcol/><ml:id labels="VARIABLE">M</ml:id><ml:real>0</ml:real></ml:apply>
  <ml:matrix rows="2" cols="1"><ml:real>4</ml:real><ml:real>5</ml:real></ml:matrix>
</ml:define>
```

전역은 바깥 태그가 `globalDefine`, 프로그램 내부 대입은 `localDefine`이다. RHS의 명시적 `eval`은 지역·전역 정의의 두 번째 child에만 둔다. 프로그램 대입에 worksheet evaluation을 중첩하는 정책을 만들지 않는다.

ignored `artifacts/rea/array-slice-authoring-decode.py`는 이전 REA raw body 위치에서 바이트를 읽고 artifact/IL SHA를 검증한 뒤 dncil로 해석한다. `array-slice-authoring-cil.json`/`.txt`가 그 결과이고 원본 코드를 실행하지 않는다. 도움말 identity는 `array-slice-authoring-help-identities.json`에 보존했다.

| 설치 `Help/en/PTC_Mathcad_Help/` | SHA-256 | 증거 범위 |
| --- | --- | --- |
| `row_and_column_operators.html` | `a870677758a3602c0fc4deabcefd06511fb08817a03a39a46b1eca9e61fa5b2e` | 전용 행·열 연산자, Ctrl+Shift+R/C, dimensionless integer 및 ORIGIN |
| `methods_for_defining_arrays.html` | `fea85703790059de1d630f1ec95034dd5d3b80faddf3794dda29cba98271c2f9` | 원소 index 대입과 미지정 원소의 zero fill; 행·열 block RHS shape의 증거는 아님 |
| `local_assignment_operator.html` | `8a66add0cd7fcb5d25f9960d0ea71d0979ff7782419bf33b14eaee81aabd1f30` | 프로그램 내부의 local scope; 행·열 LHS의 긍정 증거는 위 descriptor whitelist |

도움말의 추출 설명만으로 setter의 RHS shape를 추정하지 않았다. native setter가 받는 값과 상위 typing에서 실제로 허용하는 조합의 차이는 계산 조사에 남긴다.

## 독립 Rust 구현

기존 `NodeKind::Row/Column`과 두 operand를 그대로 사용한다. 지역 정의 삽입은 기존 root wrapping 경로를 유지하고 전역 정의 삽입의 whitelist에 named Row/Column을 추가했다. 두 경우 모두 base name의 literal suffix, explicit label, contextual flag와 원래 index tree를 보존한다. 전역 삽입은 하나의 checkpoint이며 Undo/Redo로 원래 subtree를 복원한다. SpecTable의 expression-only context와 matrix range 모드의 정의 금지 계약은 유지한다.

PortableTree validation은 지역·전역 정의 및 프로그램 LocalDefine의 named Index/Row/Column 대입을 허용한다. For binder는 계속 이름이어야 한다. 임의 계산식이나 수치 literal을 array의 base로 받는 LHS는 이 독립 구현의 지원 밖이며 raw 보존 경계를 유지한다. 원본 whitelist가 MATRIX까지 허용한다는 사실을 이름 행렬 destructuring이 구현됐다는 보고로 확대하지 않는다. 프로그램 함수 선언도 이번 확장의 구현 범위가 아니다.

bridge의 `definition_identifier`가 세 종류의 array target에서 base 이름을 찾는다. worksheet의 전역 label prepass도 이를 사용하므로 뒤에 등장하는 전역 slice 이름이 위쪽 질의에서 단위 이름으로 오인되지 않는다. `structural_label`의 과거 `row`/`col` 태그 비교는 실제 native `matrow`/`matcol`로 교정했다. 프로그램 local assignment 이름은 해당 Program node의 overlay에서만 조회하고 바깥 식으로 누출하지 않는다. 이 overlay는 독립 구현의 bounded label 선택 정책이며 원본의 모든 contextual label/type inference를 복원한 것이 아니다.

reader는 기존처럼 standalone 구조 clipboard fragment도 읽을 수 있다. native writer는 LocalDefine의 Program host를 요구한다. reader의 구조 허용과 worksheet의 실행 허용을 혼동하지 않는다.

## 확인한 회귀

editor 회귀 세 개는 named Row/Column 지역·전역 정의의 단일 Undo/Redo와 metadata, expression-only 거절 및 non-named 지역·전역 no-op, 프로그램 named Index/Row/Column 허용과 For binder 제한을 검사한다. editor 전체 68개가 통과했다.

`mathcraft-compat/tests/array_slice_bridge.rs`의 다섯 회귀는 native 지역·전역 roundtrip과 RHS evaluation·structured identifier, 잘못된 base·arity·속성·namespace 및 중첩 정의 거절, index의 독립 label 및 전역 이름의 앞쪽 참조, 프로그램 assignment roundtrip과 writer context, 프로그램 local label의 바깥 scope 누출 방지를 검사한다. 수치 결과, native worksheet 저장/열기, PDF 및 Windows·peets의 통합 검증은 상위 통합 기록에 연결한다.
