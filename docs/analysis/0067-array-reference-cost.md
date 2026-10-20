# 큰 배열 참조와 함수 scope 복제 비용

2026-10-16. Rust 계산기의 반복 index 조회·range 출력·캡처 함수 호출을 CPU headless 테스트로 재현하고 수정했다. 원본 앱·GUI·스크린샷·CLR·COM은 실행하지 않았다. [0063](0063-indexed-definition-contract.md)의 원소 정의·단위·alias·ORIGIN 계약은 유지한다. 아래의 Arc 사용은 독립 Rust 구현의 선택이며 원본 MatVal의 메모리 모델을 정확히 복제했다고 주장하지 않는다.

## 원인

기존 `Scope::lookup`은 값을 소유해서 반환했다. 일반 `Binding::Value`도, private fill dimension을 가진 `Binding::IndexedArray`도 조회할 때 `Value::Matrix.elements: Vec<Quantity>` 전체를 복제했다. `Expression::Index`는 그렇게 받은 배열에서 scalar 한 칸을 꺼냈다. 배열 N개 원소를 N번 조회하면 필요 없는 원소 복제량이 N²에 비례했다.

함수에는 비용이 하나 더 있었다. 캡처된 `Scope`를 호출마다 clone하여 parameter를 넣었고, 이 clone도 모든 배열 payload를 복제했다. `validate_names`와 `collect_range_names`의 이름 존재/분류 검사 역시 owned lookup을 사용했다. 체크포인트·취소 rollback의 binding 저장/복원도 같은 payload를 따로 소유했다.

16,384개 `Quantity`는 원소 데이터만 약 1.125 MiB다. 16,384번 전체 복제하면 한 조회 경로에서 약 18 GiB의 불필요한 복사에 해당한다. 이는 계산 결과의 복잡성이 아니라 Rust의 값 소유 경계 때문에 생긴 비용이다.

## 변경

공개 `Value`의 enum, `Vec<Quantity>`, JSON, 물리 단위 표현은 바꾸지 않았다.

- Scope의 private binding map은 immutable `Arc<Binding>`을 보관한다. 다른 모듈의 기존 `get/insert/contains_key` 사용 형태는 유지한다. Scope clone은 이름과 Arc를 복제하고 배열 원소는 공유한다.
- binding은 공유 참조를 통해 수정하지 않는다. 원소 정의는 기존처럼 임시 배열을 만들어 완성한 다음 새 binding을 한 번에 게시한다. 이전 alias·캡처·체크포인트의 배열은 그대로 남는다.
- named 배열의 index 조회는 `Cow::Borrowed(&Value)`로 scalar 한 칸을 읽는다. 계산식이 만든 임시 배열은 기존 evaluator의 owned 결과로 처리한다. 단일 index의 열벡터 제한과 두 index의 행/열 의미를 바꾸지 않는다.
- 실패 binding 검사는 참조를 빌려 수행하되 기존 fault code와 `previous_region`을 보존한다. 이름 존재 검사와 range 이름 수집은 큰 값을 복제하지 않는다.
- 함수 호출은 body/parameter 목록을 읽기 전용으로 참조하고 캡처 Scope의 immutable payload를 공유한다. parameter와 프로그램 지역 변수는 호출자의 binding을 수정하지 않는다.
- checkpoint delta, global/context 복원, 취소 rollback도 같은 immutable binding 버전을 보존한다. indexed array의 `zero_dimension`은 Value와 함께 공유된다.
- 일반 matrix alias와 indexed alias는 같은 버전을 공유할 수 있다. 이후 쓰기는 새 버전으로 분리된다. alias의 빠른 경로에도 취소 검사를 두어 이미 중단된 작업이 새 이름을 게시하지 않게 한다.

공개 API가 소유한 전체 matrix 결과를 요청하면 기존처럼 독립된 `Value`를 받는다. 호출자가 결과의 원소를 수정해도 계산 Scope의 공유 배열은 바뀌지 않는다.

## 실제 계측

Windows Cargo test profile(자체 코드 opt-level 1, dependency opt-level 2), 같은 benchmark의 단일 전후 실행이다. native 창·GPU 없이 실행했다. 이 수치를 release 앱의 보장 시간이나 원본 Mathcad와의 성능 비교로 해석하지 않는다. OS allocator/cache 상태 때문에 실행 시간의 비율이 정확한 N² 곡선을 이루지는 않지만 전체 Vec 복제라는 구조적 원인은 소스로 확인된다.

benchmark는 N×1 numeric matrix를 사용한다. 직접 조회는 일반 binding과 indexed binding 각각 N번, 함수는 캡처한 배열의 index를 N번 조회한다. range 출력은 N개 scalar를 계산한다. read-only context 경로는 실제 plot query와 같은 `CalculationContext::evaluate_mathml`에 50개의 query를 보낸다. 반환 값·원소 수·마지막 값도 함께 검사한다.

| 배열 원소 | 작업 | 수정 전 | 수정 후 |
|---:|---|---:|---:|
| 1,000 | 일반 binding index 1,000회 | 1.902 ms | 0.494 ms |
| 1,000 | indexed binding index 1,000회 | 1.790 ms | 0.446 ms |
| 1,000 | 캡처 함수 1,000회 | 58.497 ms | 1.309 ms |
| 1,000 | range scalar 1,000개 | 8.383 ms | 0.749 ms |
| 8,000 | 일반 binding index 8,000회 | 208.739 ms | 3.110 ms |
| 8,000 | indexed binding index 8,000회 | 116.240 ms | 2.944 ms |
| 8,000 | 캡처 함수 8,000회 | 3,370.299 ms | 10.072 ms |
| 8,000 | range scalar 8,000개 | 108.278 ms | 5.534 ms |
| 16,384 | 일반 binding index 16,384회 | 4,663.194 ms | 6.244 ms |
| 16,384 | indexed binding index 16,384회 | 4,768.007 ms | 6.012 ms |
| 16,384 | 캡처 함수 16,384회 | 10,495.866 ms | 17.224 ms |
| 16,384 | range scalar 16,384개 | 4,658.109 ms | 13.480 ms |
| 16,384 | read-only context query 50개 | 29.130 ms | 0.696 ms |

재현:

```powershell
cargo test -p mathcraft-compat --lib benchmark_large_array_reference_scaling --locked --offline -- --ignored --nocapture
```

ignored 분석 로그:

- `artifacts/rea/array-reference-before-20261016.log`
- `artifacts/rea/array-reference-after-20261016.log`
- `artifacts/rea/array-reference-compat-tests-20261016.log`
- `artifacts/rea/array-reference-focused-final-20261016.log`
- `artifacts/rea/array-reference-clippy-20261016.log`

## 정합성과 남은 비용

새 회귀 5개는 큰 배열의 Scope/alias가 같은 immutable 버전을 공유하면서 이후 원소 쓰기·확장은 분리됨, mixed units와 처음의 zero-fill dimension 보존, 함수의 이전 정의 시점 capture, 공개 owned 결과의 수정 격리, 음수 ORIGIN/잘못된 index/상위 실패의 오류 순서와 위치, 임시 계산 배열·행벡터 제한, 취소된 조회/alias 게시 금지, checkpoint의 정확한 이전 버전 복원을 검사한다.

compat 전체 166개가 통과했고 실패는 0개다. 2개 ignored 중 새 배열 benchmark는 직접 실행해 통과했다. 마지막 public owned 결과 수정 검사도 별도로 5개 focused 회귀를 다시 실행해 통과했다. `cargo clippy -p mathcraft-compat --all-targets --locked --offline -- -D warnings`도 통과했다. 기존 indexed 정의·전역 충돌·함수 capture·프로그램·solver·취소 회귀를 포함한다. 양OS 전체 통합 검증은 이후 동일 source archive에서 수행한다.

이 변경이 모든 배열 비용을 상수 시간으로 만들지는 않는다. 실제 matrix 연산, 새 원소 정의의 atomic 임시 배열 작성, 공개 전체 결과 게시, display 단위 변환에는 데이터 크기에 비례한 작업이 남는다. Scope clone은 여전히 binding 이름/Arc를 담은 HashMap을 복제하므로 이름이 매우 많은 함수 호출은 별도 비용이 있을 수 있다. 원본의 전체 dependency graph, 배열 최대 크기, 모든 값/동적 함수 의미가 구현되었다는 뜻도 아니다. 독립 엔진의 16,384 원소 budget과 기존 미지원 경계는 유지한다.
