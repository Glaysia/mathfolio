# 표 열 정의의 계산 통합

2026-10-16. Mathcad Prime 12.0.0.1의 설치 산출물을 정적으로 조사하고 독립 Rust 엔진으로 검증했다. 원본 앱·GUI·스크린샷·CLR·COM 실행은 사용하지 않았다. 원본 코드의 복사나 실행 없이 구조·호출 관계·파일 계약만 구현에 사용했다.

## 확인한 원본 계약

설치된 도움말 `Help/en/PTC_Mathcad_Help/about_tables.html`은 표를 입력 전용으로 정의한다. 첫 행은 열 이름, 두 번째 행은 단위이고 각 데이터 집합을 일반 변수처럼 함수·그래프에서 사용할 수 있다. 원본 예제에는 단위가 없는 `scale(matrix, placeholder)`도 있다. `to_insert_a_table.html`의 표 길이는 가장 긴 데이터 집합을 기준으로 하므로 짧은 열이 있는 직사각형 표를 허용한다. 도움말 SHA-256은 각각 `8bebe8eaa0c17b8dbd8ba63b37af07be679b2c76685062f86c2355a982eb9304`, `0f1b542f4b560d6f5c02d30c700d7f5e7eab38e3e5c2deb7a77dc8b27339d3f0`이다.

정적 근거의 원본은 `Ptc.EquationEditor.dll` SHA `d419032c0e99a62eab2f278e571a338d8c4b9662655af8ab462d482a028c0223`, REA evidence `ev_419ab24312df266f2e58ff6b91d32e0261a9a2bba9105ba09ebecb7517b43fdf`이다. retained member metadata와 원본 파일의 SHA 및 각 IL SHA를 검증한 읽기 전용 decoder는 ignored `artifacts/rea/spec-table-contract-decode.py`, 출력은 `spec-table-contract-cil.json`이다.

| 메서드 token | 관찰 | IL SHA-256 |
| --- | --- | --- |
| `0x06000a4f` | 열의 데이터 셀 개수가 1이면 scalar, 아니면 `MATRIX(count, 1, data)`를 만들고 scaling expression을 적용한다. | `38188688689a2e33cbde53362da6cdaaa1c8c2119affc8d4e9910dc64f8d934f` |
| `0x06000a46` | 데이터 셀 전체의 S-expression을 순서대로 결합한다. 이 함수 자체에는 trailing placeholder 제거가 없다. | `4898bda83feb571318d363ecd6efe8e5d046705fdb35f2465b548f6a2e36795e` |
| `0x06000a4e` | 열 이름 정의와 scaled data expression을 결합한다. | `0ed98b91b5e6c4b33d9a130861e348cd30b17e5dbbe4424f30bd6e2dcf87508d` |
| `0x06000aae` | placeholder인 scaling cell과 뒤에서 연속한 빈 데이터 셀을 extra로 분류한다. 헤더 및 첫 데이터 셀의 빈칸은 extra가 아니다. 내부 빈칸까지 무시하지 않는다. | `ebcbc597e7df71e2a3db8d87e63b389b63599d5c269226435d004c9cd6f42ef0` |
| `0x06000a52` | 원본 XML reader는 일반 `define` 아래 `scale`, `matrix cols=1 rows>0`를 읽는다. 직렬화된 행을 모두 셀 모델로 읽는다. | `696c3d564f1953c4de36ccd65f6241c43a1b5631fa4b9c4f9041448a5629e2fc` |
| `0x06000a53` | writer는 직사각형 표의 padding을 포함한 matrix를 저장한다. | `461241340955651807a7a064230cc53cdcfe59086d221c0a5b07b61eca84db38` |
| `0x06000a12` | 각 열 wrapper의 Commit이 자신의 modified 상태를 해제하고 RequestEvaluation을 호출한다. | `426cb6b3476034133dd6a2e50082ef7fa18ccc6e6cb7711c2ba3f2c2aa0a7784` |
| `0x06000aad` | 표 Commit은 열 wrapper를 반복하며 각각 Commit한다. | `95987015743fb587ad0cfdef27b8e35237b63b137b70d7985e0888350aaf148b` |

`CreateMatrixFromTableCells`, token `0x06000af6`, IL SHA `7d7b538f8ee83bdf5e2adda157053302553b4982e562f6c5a7d73ecc7fcc0871`은 선택된 사각형 셀을 순회하여 복제하는 경로다. 이것을 표 계산 시 padding 제거의 직접 근거로 사용하지 않는다.

`Ptc.EquationControl.dll` SHA `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`, evidence `ev_389338a9845c7ea83dfd310922316cf92ca1637bdbac6602a01c4690e52b8349`의 `0x060002cd`는 표 모델의 Commit으로 위임한다. 원본 입력 표와 계산 결과 출력의 역할을 섞지 않는다.

## 독립 구현과 경계

`mathcraft-compat/src/spec_table.rs`는 `<region><spec-table><math>…`의 열 정의를 읽고 같은 계산 Scope에 순서대로 등록한다. 외부 행은 표의 기존 region ID 하나이며 열마다 임의 ID를 생성하지 않는다. 표 자체의 숫자 결과는 없다. 이후 수식은 열의 물리 단위가 있는 vector 또는 scalar를 참조한다. 명시적인 결과 단위 slot은 물리 binding을 변경하지 않는다.

빈 단위 slot은 항등 scale로 해석한다. 연속한 trailing placeholder를 줄이고 실제 데이터가 한 셀이면 scalar로 내린다. 전체가 빈 열에는 첫 빈 셀을 남겨 오류를 내며 내부 빈 셀도 오류다. 빈칸을 0으로 채우지 않는다. **원본의 extra-cell 분류와 longest-data-set 도움말은 이 정책을 뒷받침하지만, 원본 native evaluator가 padding을 제거하는 최종 단계와 정확한 vector 길이는 정적 자료에서 아직 직접 확인하지 못했다.** 이 부분은 문서화된 독립 정규화이며 완전한 원본 엔진 동작 복원이라고 주장하지 않는다. 원본 저장 XML의 padding은 문서 계층에서 그대로 보존한다.

열별 wrapper 계약에 맞춰 한 열의 일반 계산 오류가 있어도 다른 열의 정의는 처리한다. 표 행에는 첫 오류와 열 번호를 집계한다. 원본과 동일한 개별 셀 오류 위치·오류 문자열을 구현했다는 뜻은 아니다. 잘못된 컨테이너/열 구조는 거부한다. plain matrix를 직접 자식으로 두거나 함수 header를 쓰는 비원본 표 형식은 지원하지 않는다.

`editor_bridge::worksheet_xml`은 일반 수식과 표를 동일한 위치 순서로 label 해결한다. 표 이름이 `m`이면 다음 수식의 자동 label이 변수 m을 가리키지만, 표의 단위 slot과 명시 UNIT m은 여전히 단위다. `disable-calc=true/1`인 표는 계산과 후속 label 선언에서 제외한다.

표가 바뀌면 모든 열의 binding을 같은 checkpoint delta에 보관한다. ThroughRegion/F5의 외부 대상은 표 ID이며 열 전체를 하나의 계산 단위로 처리한다. 취소되면 표의 bindings 및 행 상태를 전부 rollback하고 pending으로 남긴다. 이 원자적 취소 정책은 독립 엔진의 기존 region 경계에 맞춘 선택이다. 원본은 열별 wrapper를 사용하므로 원본 scheduler와 동일한 취소 입도라고 주장하지 않는다.

## 실제 파일과 검증

사용자가 제공한 `1015_LCC_S.mcdx` SHA `217994a631de58e096a3cfdb8a8251aca91501e62151f869950b99f738fc18b5`와 `1016_LCC_S.mcdx` 스냅샷 SHA `9385cd1c4f2ce7a0e23d7474560040f6fde886e06c8071e634350a7866005932`의 region 26은 동일한 6열 표다. 각 열은 저장 시 10행이고 숫자 55개와 padding placeholder 5개를 포함한다. `I_dc` 10개/A, `I_rms` 10개/A, `A_mm2` 10개/mm², `D_mm` 10개/mm, `ΔT_fea_K` 8개/K, `awg` 7개/A이다. rich subscript 이름은 표시 문자열을 단순 연결하지 않고 기존 identifier identity를 유지한다.

각 열 resultRef 18–23의 원본 저장 cache는 Synchronized 상태만 있고 numeric payload가 없다. 이는 입력 전용 표와 일치한다. 표 밖 수식 79개의 성공만으로 여섯 열의 계산을 검증했다고 하지 않는다. 별도의 downstream 참조 테스트가 필요하다.

회귀 테스트는 `tests/spec_tables.rs`의 55개 값/5개 빈칸, 차원 및 display override, 한 셀 scalar, 내부·전체 빈칸 오류와 다른 열의 성공, geometric label/disabled 경계, 수정 후 전체 checkpoint replay를 포함한다. 이전에 같은 이름으로 99를 정의한 뒤 표의 내부 빈 셀이 실패하면 다음 참조도 실패하며, 계산 checkpoint를 재생해도 99가 되살아나지 않는 경계도 검사한다. 내부 빈칸의 오류 유형·문자열이 원본과 정확히 같다는 검증은 아니다. 모듈 내부 테스트는 두 번째 열 계산 중 취소 후 전체 표 재개를 검증한다.

Windows에서 전체 `cargo test -p mathcraft-compat --locked --target-dir target/compat-spec-tests`가 통과했다(기존 성능 benchmark 1개는 ignored). 마지막 경계 테스트 추가 후 focused `--test spec_tables` 6/6도 통과했다. 표 내부 취소 테스트 1개 역시 통과했다. 별도 target은 다른 작업의 앱 release 빌드 lock을 기다리며 큐를 늘리지 않고 순수 compat 검증을 진행하기 위해 사용했다.

확장한 ignored `unit-regression-probe`는 두 실제 파일 각각 최상위 math 79개와 표 region 하나, 총 80개 외부 행을 같은 bridge로 계산했고 오류가 0이었다. 원래 rich header를 그대로 사용하는 별도 downstream query 6개씩을 추가하여 길이 `10/10/10/10/8/7`을 확인했다. `spec-table-report.py`는 원본 ZIP에서 읽은 각 숫자와 계산된 숫자 55개를 모두 비교하고 mm²→m²의 `10⁻⁶`, mm→m의 `10⁻³`, A/K의 차원을 확인했다. padding 5개가 숫자 0으로 추가되지 않으며 물리값 비교 허용 오차는 상대 `10⁻¹²`, 절대 `10⁻¹⁵`이다. 이는 독립 엔진의 원본 입력 이행 검사이며 원본 Mathcad 재실행 결과와의 비교는 아니다. 상세 JSON은 ignored `artifacts/rea/spec-table-calculation-report.json`, `spec-table-1015-calculated.json`, `spec-table-1016-calculated.json`에 있다.
