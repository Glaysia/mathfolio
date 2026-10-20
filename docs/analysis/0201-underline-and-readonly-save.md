# 읽기 전용 저장·선택 밑줄·그래픽 관측 경계

2026-10-20 r213 고정 기록. 원본/GUI/CLR을 실행하지 않았다. 기능의 부분 구현 깊이·검사 수를 40% 달성이나 체감 지연 개선으로 환산하지 않는다.

readonly Current native 저장은 metadata regular-file readonly 양성에만 질문/Save As를 연결하며 취소는 draft/cache/history 준비 전이다. ACL·lock·임의 오류 분류/재시도와 실제 modal IME는 미검증이다.

선택 U는 명시 None/Underline 범위다. imported absent는 unknown, authored의 검증된 생성 scaffold 기본값은 별도 정책이다. toolbar의 혼합 retained 값과 keyboard mixed→Underline은 구분한다. focused worksheet Text의 미지원/IME/빈 선택 Ctrl+U는 egui 문단 삭제 fallback을 막기 위해 소비하며 source/history를 바꾸지 않는다.

Basic Render Driver Cpu/Dx12 headless 관측은 실제 GUI adapter/지연과 다르다. release requested flags32에서도 강제 제출 완료 약195ms가 남아 DEBUG shader 가설의 개선은 확인하지 못했다. locked wait chain에 고정200ms polling quantum은 없었다. GPU 원인·GUI FPS·픽셀 parity를 단정하지 않는다.

[원본 U 정적 근거](../../artifacts/rea/rich-text-underline-core-next-20261020/README.md), [readonly 원본 근거](../../artifacts/rea/readonly-save-prompts-next-20261020/README.md), [독립 밑줄 검사기](../../artifacts/rea/underline-validator-next-20261020/README.md), [graphics 관측 제한](../../artifacts/rea/wgpu-dx12-wait-next-20261020/README.md)를 구분한다.

## 고정 검증

| 항목 | 근거 |
|---|---|
| windows | 1850 |
| peets | 1851 |
| ignored_windows | 100 |
| ignored_peets | 98 |
| mcdx | 192 |
| standalone_xml | 10 |
| pdf | 120 |
| xsd_parts | 1162 |
| underline_own | 12 |
| underline_observations | 2 |
| archive SHA | `07bf6069bf6ad4a373f6f4aa9069ec5731e1f16e468e1dfdb73ad31802d8a81a` |
| manifest SHA | `5eccbb483c00c5c1d6980ba73b9ba060c8883db3f328d8e1bb23266ce3e8d937` |
| accepted validation SHA | `d7b4cea9d5e4a8bd9285f6090dce1cdfec45f0729e5611c3c9581b5280e02fa4` |

Underline own12/observation2는 독립 literal source와 cold package/PDF vector 검사로 결합했다. 일반 source/원본 serializer 동일성이나 빈 PDF의 underline ink parity는 주장하지 않는다.

r212는 두 파일 fmt에서 중단되어 tests/probes를 실행하지 않았다. 두 파일 rustfmt 뒤 새 r213 cohort로 다시 검증했다. r213 최초 validator는 Git의 CRLF 변환을 exact SHA guard가 거절했으며, reviewed LF를 그대로 복원하고 양 semantic 검사를 새 로그로 재실행했다. product/의미 predicate 변경이나 재빌드는 없었다. 검사 수는 retry 합산이 아니다.

기계 판독 이력은 이 문서에 복제하지 않고 다음 원문 receipt에 보존한다.

- [accepted validation 및 전체 이전 이력](../../artifacts/rea/underline-integration-20261020-r213/validation.json)
- [r212 fmt 실패 cohort](../../artifacts/rea/underline-r213-preparation-20261020/r212-fmt-failure-receipt.json)
- [r213 metadata 줄바꿈 복원](../../artifacts/rea/underline-r213-validator-line-ending-repair-20261020/receipt.json)
- [독립 밑줄 XML/source 결과](../../artifacts/rea/underline-integration-20261020-r213/underline-xml-validation.json)
- [독립 밑줄 PDF/vector 결과](../../artifacts/rea/underline-integration-20261020-r213/underline-pdf-validation.json)
