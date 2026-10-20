# 창 없는 그래픽 어댑터 조회

r208 개발 조사다. 조회 테스트 코드는 [r208 양 OS 고정 검증](0197-rich-paragraph-source.md)에 포함되지만 실제 어댑터 조회는 아래 별도 Windows 개발 harness에서 한 번 실행했다. 사용자 GUI·스크린샷·설치 Mathcad 실행을 사용하지 않는다. 렌더러 설정이나 드라이버를 바꾸지 않았다.

[Windows 읽기 전용 조회](../../artifacts/rea/windows-video-controller-readonly-20261020.json)는 Microsoft Basic Display Adapter/SeaBIOS VBE와 Microsoft Remote Display Adapter를 보고한다. CIM의 refresh 값은 앱 프레임 속도를 측정한 값이 아니다. [peets 장치](../../artifacts/rea/peets-display-controller-readonly-20261020.log)는 GeForce GT220 GT216을 보고하지만 [드라이버 조회](../../artifacts/rea/peets-display-driver-readonly-20261020.log)에 Kernel driver in use 행은 없었다. 이를 Linux 하드웨어 가속 확인이라고 해석하지 않는다.

Windows 전용 ignored 자체 테스트는 DX12/Vulkan Instance 생성과 adapter 조회만 한다. GL은 Instance 생성 전에 제외한다. locked wgpu-hal의 WGL 경로에는 helper HWND 생성 코드가 있어 GUI 금지 조건과 맞지 않기 때문이다. surface·device·queue·render·submit·image readback을 요청하지 않는다. 외부 실행기는 CreateNoWindow와25초 종료 한도를 사용한다. 이 코드 경로의 창 생성 요청 부재를 opaque 드라이버 내부까지 포함한 전역 HWND 감사라고 부르지 않는다.

[어댑터 결과](../../artifacts/rea/graphics-r208-adapters-20261020-a/wgpu-adapters.json)는 Microsoft Basic Render Driver 하나, DX12, DeviceType Cpu를 보고했다. 기본 headless 선택도 같고 actual_gui_selected는 null이다. [실행 receipt](../../artifacts/rea/graphics-r208-process-20261020.json)와 [로그](../../artifacts/rea/graphics-r208-probe-20261020.log)는 자체 test harness의 조회 성공을 고정한다. 현재 앱 창이 같은 어댑터를 선택했다고 단정할 수 없다.

소프트웨어 렌더링 경로가 노출된다는 근거이며 실제 버벅임의 전부가 GPU 때문이라는 증명은 아니다. NativeOptions 기본은 LOW_LATENCY, AutoVsync, desired maximum frame latency1이다. 하위 WgpuConfiguration의 HIGH_THROUGHPUT 기본만 보고 앱이2프레임을 의도적으로 쌓는다고 설명하면 틀린다. test build validation flag와 headless 선택도 shipping GUI 상태와 구분한다. 후속 offscreen 측정이 가능하더라도 OS presentation·입력 후 화면 표시 지연과는 별개다.
