# Own-format의 보존 native 결과 복원과 전체 context 검증

2026-10-19. 계산 cache가 없는 `.mathcraft`가 이전 native archive를 보존하고 있을 때의 cold restore 수정이다. GUI·스크린샷·원본 runtime·CLR을 실행하지 않았다. Mathcraft 자체의 source·headless import/restore/export 테스트와 opt-in CPU probe만 사용했다. 단위 표시의 source association 검증과 별개로, **상위 계산 문맥 전체가 현재 source와 같은지** 확인해야 하는 문제다.

## 재현된 문제

기존 open은 실제 `.mcdx` import로 만든 document와 `.mathcraft`에 남은 archive를 모두 `restore_saved_calculations_with_package(Some(package))`에 전달했다. valid own cache가 있으면 해당 source signature를 사용했지만, `calculation_cache` 자체가 없으면 native fallback으로 들어갔다.

XY CacheIndex는 plot literal source, trace ordinal/resultRef, 캐시 형식·상태를 검사했다. 그러나 `gain:=2`를 `gain:=7`로 고친 own-format에 옛 archive만 남고 curve source `gain*x`는 그대로면, plot 자체는 같다고 판단하여 이전 curve가 Synchronized/Ready로 복원될 수 있었다. 일반 Math fallback은 resultRef로 직접 저장값을 찾았고, Area fallback도 개별 child Math 일치만 검사하여 upstream 변경을 증명하지 못했다. 마지막 Area 복원 branch에서 다시 Sync를 넣을 수 있어 XY 행만 낮추는 수정은 충분하지 않았다.

이제 own cache가 없는 retained archive 복원에는 전체 source/context proof가 필요하다. false/Err이면 값은 이전 표시로 보존하면서 Pending으로 낮춘다. 파일 열기에서 새 계산은 실행하지 않는다.

## 공유한 source proof

`mathcraft_document::export::source_context_matches(&CanvasDocument, &NativePackage) -> Result<bool,String>`를 추가했다. caller는 해당 canvas가 실제 보존한 archive에서 읽은 package를 넘겨야 한다. 임의의 두 문서 사이 engine 결과의 진실성을 증명하는 API가 아니다.

기존 `export_into`의 source 작성 부분과 기존 `unique_results / unique_references / ContextSnapshot::matches / same_calculation_program` 검사를 `project_source`로 공유했다. 일반 export는 그 뒤에만 cache invalidation, 새 Pending result row 추가, saved/plot result 쓰기를 수행한다. proof는 ephemeral package copy에서 source projection과 검증만 수행하고 그 복사본을 버린다. 기존 reference count map도 돌려주므로 정상 export에 불필요한 재parse를 추가하지 않았다.

proof에는 파일 IO, ZIP 재압축/재파싱, evaluator/worker 제출, cached MathML 실행, result part 재작성, 식의 임의 label 재추론이 없다. 자체 typed→native source writer를 두 번째로 복제하지 않는다. `NativePart`와 `NativePackage`에 Clone derive만 추가해 decoded bytes를 일시 복사한다. settings/relationships/header/footer/flow/binary 등 결과 part 이외의 package context도 기존 ContextSnapshot의 엄격한 raw-byte hash 계약을 유지한다. 표/Area/opaque/알려지지 않은 source의 검증이 실패하면 Sync로 우회하지 않는다.

## 앱 연결과 상태 보존

새 `worksheet_ui/plot_restore_context.rs`는 다음을 제공한다.

* `may_restore_synchronized(document, package)`: archive 존재와 native presentation에서 유도한 grid/page size를 확인한 뒤 공유 source proof를 호출한다. canvas에만 바뀐 grid/size를 옛 package 설정으로 조용히 덮지 않는다.
* `invalidate_unproven(app, package)`: own cache가 없고 archive가 있을 때만 검사한다. proof false/Err이면 복원된 계산 행·Area child를 Pending으로 낮추고 활성 owner를 pending set에 넣는다. 일반 source/값/표시/오류/경고를 계산 input으로 바꾸지 않는다.

Ready curve는 `Pending { previous: Some(curve) }`로 옮긴다. 기존 Pending, Failed, Incomplete, Unavailable의 진단·빈 슬롯은 유지한다. native-origin `NativeMathml` numeric_source marker는 지우고 serialized display와 raw points/단위 메타데이터는 보존한다. Area 부모의 inert bundle도 Pending child 상태로 다시 정규화하여 own 저장·재열기에서 nested Sync가 부활하지 않게 했다. 재정규화에 실패한 source는 bundle을 새 Sync 근거로 보존하지 않는다.

root의 중앙 연결은 전체 일반/plot/Area 복원 **뒤**, calculation_pending를 정하기 전에 이 helper를 호출한다. 기존 유효 own saved cache 경로, archive 없는 일반 문서는 이 추가 proof를 하지 않는다. source signature가 불일치하는 이미 존재하는 own cache를 native cache로 대체하는 정책도 추가하지 않는다.

## 실제 fresh native import의 신뢰 경계

전체 source projection은 큰 문서에서 비싼 작업이다. root는 `open`에서 own archive를 decode하기 **전**의 `fresh_native`를 보존하고 `restore_saved_calculations_for_source(package, fresh_native)`로 전달했다. 실제 `import_native`가 같은 package에서 document를 방금 생성한 경로만 trusted fresh origin이다. 이 경로는 마지막 retained-archive proof를 건너뛴다. 원래의 native source/reference/status/diagnostic 검사는 계속 수행하며 계산하지 않는다.

`.mathcraft`에 archive가 존재한다는 사실이나 `Some(package)`만으로 fresh로 취급하지 않는다. 일반 `restore_saved_calculations_with_package` wrapper의 기본값은 false다. 따라서 edited own-format은 전체 proof를 요구하고, 실제 fresh native open에 아래154ms 비용을 새로 부과하지 않는다. 이 origin의 정확성은 importer와 restore 사이에 source를 편집하지 않는 중앙 경로의 계약이며, 모든 archive를 무조건 신뢰한다는 뜻이 아니다.

## Headless 회귀

새 helper의 일반 테스트5개가 통과했다. counter는 cfg(test) thread-local이고 제품 clock/counter 비용은 없다. 기존 legacy geometry 통합 회귀도 별도로1개 통과했다.

| 회귀 | 확인 내용 |
| --- | --- |
| source_proof_is_in_memory_result_inert_and_rejects_upstream_changes | 실제 native 생성→open, 동일 source true / gain 변경 false; 원본 package의 모든 part bytes 동일; 계산 실행0 |
| exact_own_archive_restores_zero_queries_but_changed_upstream_and_area_stay_pending | 일반·Area 양쪽 실제 own save/open: 동일 archive는 정상 결과/실행0, upstream 변경은 Math scalar query·XY·Area Pending; 이전 수치·curve좌표 보존; 빈 frame2개 후 실행0; 다시 save/open해도 Pending |
| proof_binds_grid_order_deletion_disabled_and_external_page_context | grid 변경, 계산 순서 변경, upstream 삭제, disabled 변경, page size 변경은 Sync 복원 불허 |
| exact_legacy_authoring_paper_representation_still_requires_whole_context_proof | legacy own-format의 정확한 f64 A4 크기는 허용하되 임의 nearby f64 및 upstream 변경은 거절 |
| trusted_actual_native_import_skips_projection_but_own_archive_requires_one_proof | 실제 native import는 proof call0; 같은 archive의 own-format cache 없는 open은 proof call1. archive 보유 여부와 fresh origin을 구분 |

실행 기록은 `artifacts/rea/plot-restore-context-tests-20261019.log`다(5passed /1ignored /0failed). 기존 `legacy_own_model_restores_retained_curve_without_geometry_migration_but_rejects_newer_source` 회귀1개 통과는 `plot-restore-context-legacy-geometry-tests-20261019.log`에 보존했다. 첫 추가 빈-frame 테스트에서 configure하지 않은 Context 때문에 math-italic font panic이 났고, 테스트 fixture에 정상 `crate::configure`를 적용했다. 실패 시도는 `plot-restore-context-test-fixture-failed-20261019.log`에 보존했다. 제품 font/입력 버그로 보고하지 않는다.

공유 export refactor 뒤 document 전체는 **220passed /7ignored /0failed**, document all-target Clippy `-D warnings`가 통과했다. logs는 `plot-restore-context-document-{tests,clippy}-20261019.log`다. 당시 app all-target Clippy는 다른 owner가 작업 중인 scaling_field_tests의 TextBuffer trait import 오류로 중단되어, 전체 app Clippy 통과를 이 기록에서 주장하지 않는다. 소유 파일 외 오류는 해당 worker에게 전달했다.

## 기존 own-format paper 표현의 정확한 호환

첫 전체 앱 gate에서 기존 legacy geometry 통합 회귀가 Pending으로 잘못 낮아졌다. 보존된 fixture를 확인한 결과, `CanvasDocument::default`의 A4 크기는 f64 centimetre 산술값이지만 importer의 `page::source_settings`는 같은 paper를 f32 Vec2로 투영한 뒤 f64로 저장했다. 초기 helper는 후자만 exact 비교했으므로 source projection 전에 정상 legacy own-format을 거절했다. plot geometry adapter나 상위 수식 문맥의 불일치가 원인은 아니었다.

이제 알려진 native paper는 importer의 정확한 f32→f64 pair와 `PaperSize::size`의 정확한 f64 pair를 허용한다. A4 Portrait에는 기존 `CanvasDocument::default`의 정확한 centimetre 산술 pair도 보존한다. width와 height를 개별적으로 임의 조합하지 않고 pair 전체의 bit 일치를 요구한다. epsilon/오차 허용/임의 반올림을 추가하지 않았으며 모든 표현은 여전히 같은 전체 source/context proof를 통과해야 한다. nearby width16ULP 변경과 upstream gain 변경을 각각 거절하는 회귀를 추가했다. 이 호환 수정은 sourceproof/export/cache byte 의미를 변경하지 않는다.

## 추가 cold 비용 — 빨라졌다는 주장이 아님

opt-in `worksheet_ui::plot_restore_context::cold_context_proof_profile`은1/4000scalar +1Plot의 실제 native source를 만든 뒤, 기존 package와 imported canvas에 대해 proof 자체만5회 실행했다. timed block에는 source projection·decoded-part Clone·context hash가 포함되고, fixture 생성/native export/import/ZIP/파일 IO/계산은 제외된다. elapsed CPU-bound 시간이며 프로세스 CPU-time API 측정은 아니다. 첫 invocation과 뒤 invocation 모두 package copy는 매번 새로 만든다. 서로 통제된 before/after 비교나 전체 open benchmark가 아니다.

| fixture | 매 proof 복사하는 part 길이 합 |5회 elapsed ms | median ms |
| --- | --- | --- | --- |
|1scalar +1Plot|4,971B|0.7455 /0.6371 /0.5823 /0.5672 /0.5137|0.5823|
|4000scalar +1Plot|1,273,234B|149.5 /147.5071 /155.5262 /154.7487 /153.9549|153.9549|

이는 cache 없는 **own-format cold fallback**에 추가되는 검증 비용의 사례다. 최종 fresh-native origin fast path는 이 proof를 호출하지 않는다. 작은 clone만 보고 전체 비용이 작다고 주장할 수 없고,154ms의 세부 지배항이 clone인지 source serialize/XML parse/hash인지 이번 probe로 분해한 것도 아니다. 원래 underlying cache 생성/수명 문제를 해결했다고 주장하지 않는다. 이후 최적화는 같은 whole-context proof와 falsematch 방지 oracle를 유지한 별도 조사여야 한다.

복사량은 part bytes의 len 합이며 Vec capacity/metadata/allocator overhead/추가 source String·DOM/peak/RSS는 제외한다. 기존 NativePackage는 decoded total256MiB 등의 자체 예산을 갖고 있어 큰 image/binary context는 그 범위 내에서 일시적으로 복사될 수 있다. 이 추가 copy와 source projection이 끝나면 해제되며 새 persistent cache/계산 환경을 유지하지 않는다. 큰 binary archive에 대한 실측 성능은 미측정이다. per-key/paint/trace loop에 이 API를 넣어서는 안 된다.

raw는 `artifacts/rea/plot-restore-context-profile-20261019/context-proof.json`, log는 `plot-restore-context-profile-20261019.log`다. frozen app harness SHA=`8adbe4087c87248d3f1759c17384151ca3269a3984210f0413a01dac0aeab96f`. 이 harness는 counter 추가 전이며 같은 proof 제품 동작을 측정했다. 이후 cfg(test) counter/조건문 lint 정리, root fresh-origin 연결, 위의 정확한 paper 표현 호환을 추가했다. 해당 수치가 최종 소스 전체를 재측정한 것이라고 섞지 않는다. profile fixture는 imported f32 paper 표현이며 두 버전에서 같은 전체 proof를 요구한다. 기존 성능 baseline을 덮어쓰지 않았다.

미지원 source/형식·해석 실패에서는 fresh 여부를 위조하여 Sync를 되살리지 않는다. 원본과 전체 문맥이 같다는 충분한 증거가 없으면 이전 표시를 Pending으로 보여주는 것이 이 수정의 경계다.
