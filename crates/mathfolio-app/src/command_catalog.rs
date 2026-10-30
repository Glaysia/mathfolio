//! Discoverability metadata. Key dispatch stays in the existing editor/controllers.
use crate::ribbon::{RibbonAction as A, RibbonContext};

#[derive(Clone, Copy)]
pub(crate) enum Scope {
    Always,
    Selection,
    Identifier,
    Matrix,
    Plot,
    Undo,
    Redo,
    Running,
    Errors,
}
pub(crate) struct Command {
    pub action: A,
    pub category: &'static str,
    pub title: &'static str,
    pub keys: &'static str,
    pub description: &'static str,
    pub example: &'static str,
    scope: Scope,
}
impl Command {
    pub fn disabled_reason(&self, c: &RibbonContext<'_>) -> Option<&'static str> {
        match self.action {
            A::InsertArea if !c.can_insert_area => {
                return Some("영역을 묶을 수 있는 문서 위치를 선택하세요.");
            }
            A::InsertPageBreak if !c.can_page_break => {
                return Some("페이지 나누기를 지원하는 문서 위치를 선택하세요.");
            }
            _ => {}
        }
        let (enabled, reason) = match self.scope {
            Scope::Always => (true, ""),
            Scope::Selection => (c.has_selection, "먼저 문서에서 영역을 선택하세요."),
            Scope::Identifier => (
                c.can_label,
                "수식에서 이름을 선택하거나 이름 안에 커서를 놓으세요.",
            ),
            Scope::Matrix => (
                c.in_matrix || c.in_table,
                "행렬이나 표의 셀 안에 커서를 놓으세요.",
            ),
            Scope::Plot => (c.has_plot, "문서에서 도표를 선택하세요."),
            Scope::Undo => (c.can_undo, "실행 취소할 변경이 없습니다."),
            Scope::Redo => (c.can_redo, "다시 실행할 변경이 없습니다."),
            Scope::Running => (c.calculation_running, "현재 실행 중인 계산이 없습니다."),
            Scope::Errors => (c.has_errors, "문서에 표시된 계산 오류가 없습니다."),
        };
        (!enabled).then_some(reason)
    }
    pub fn tooltip(&self) -> String {
        let keys = if self.keys.is_empty() {
            String::new()
        } else {
            format!("  {}", self.keys)
        };
        let example = if self.example.is_empty() {
            String::new()
        } else {
            format!("\n예: {}", self.example)
        };
        format!("{}{keys}\n{}{example}", self.title, self.description)
    }
    pub fn matches(&self, query: &str) -> bool {
        text_matches(
            query,
            &[
                self.title,
                self.category,
                self.keys,
                self.description,
                self.example,
            ],
        )
    }
}
macro_rules! commands { ($(($a:ident,$cat:literal,$title:literal,$keys:literal,$desc:literal,$example:literal,$scope:ident)),*$(,)?)=>{
 pub(crate) static COMMANDS:&[Command]=&[$(Command{action:A::$a,category:$cat,title:$title,keys:$keys,description:$desc,example:$example,scope:Scope::$scope}),*];
};}
commands![
    (
        New,
        "파일",
        "새 계산서",
        "Ctrl+N",
        "새 문서를 엽니다. 변경된 문서는 저장 여부를 먼저 확인합니다.",
        "",
        Always
    ),
    (
        Open,
        "파일",
        "열기",
        "Ctrl+O",
        "Mathfolio 또는 Mathcad MCDX 계산서를 엽니다.",
        "",
        Always
    ),
    (
        Save,
        "파일",
        "저장",
        "Ctrl+S",
        "현재 편집 내용을 원래 문서 형식으로 저장합니다.",
        "",
        Always
    ),
    (
        SaveAs,
        "파일",
        "다른 이름으로 저장",
        "Ctrl+Shift+S",
        "새 위치와 이름으로 문서를 저장합니다.",
        "",
        Always
    ),
    (
        ExportMcdx,
        "파일",
        "MCDX 내보내기",
        "",
        "Mathcad에서 다시 열 수 있는 워크시트로 내보냅니다.",
        "",
        Always
    ),
    (
        Print,
        "파일",
        "인쇄",
        "Ctrl+P",
        "용지와 페이지 범위를 확인하고 인쇄합니다.",
        "",
        Always
    ),
    (
        ExportPdf,
        "파일",
        "PDF로 저장",
        "",
        "수식과 글자를 벡터 PDF로 내보냅니다.",
        "",
        Always
    ),
    (
        PageSetup,
        "문서",
        "페이지 설정",
        "",
        "용지 크기와 여백을 조정합니다.",
        "",
        Always
    ),
    (
        Undo,
        "편집",
        "실행 취소",
        "Ctrl+Z",
        "마지막 편집을 취소합니다.",
        "",
        Undo
    ),
    (
        Redo,
        "편집",
        "다시 실행",
        "Ctrl+Y",
        "취소한 편집을 다시 적용합니다. Ctrl+Shift+Z는 다른 수식 입력 키입니다.",
        "",
        Redo
    ),
    (
        Cut,
        "편집",
        "잘라내기",
        "Ctrl+X",
        "선택한 영역이나 편집 중인 선택을 잘라냅니다.",
        "",
        Selection
    ),
    (
        Copy,
        "편집",
        "복사",
        "Ctrl+C",
        "선택한 영역이나 수식을 복사합니다.",
        "",
        Selection
    ),
    (
        Paste,
        "편집",
        "붙여넣기",
        "Ctrl+V",
        "현재 작성 위치에 클립보드 내용을 붙여넣습니다.",
        "",
        Always
    ),
    (
        Delete,
        "편집",
        "영역 삭제",
        "Delete",
        "선택한 영역을 삭제합니다. 수식 안에서는 편집기의 삭제 동작을 따릅니다.",
        "",
        Selection
    ),
    (
        InsertMath,
        "작성",
        "수식",
        "",
        "문서의 작성 위치에 구조 수식을 입력합니다.",
        "a : 2 → a := 2",
        Always
    ),
    (
        InsertText,
        "작성",
        "텍스트",
        "Ctrl+T",
        "설명과 제목을 쓸 텍스트 영역을 만듭니다.",
        "설계 조건",
        Always
    ),
    (
        InsertTable,
        "작성",
        "표",
        "",
        "셀과 열 정의를 가진 표를 삽입합니다.",
        "",
        Always
    ),
    (
        InsertPlot,
        "작성",
        "XY 도표",
        "Ctrl+2",
        "계산된 데이터나 수식을 XY 도표에 연결합니다.",
        "",
        Always
    ),
    (
        EditPlot,
        "도표",
        "도표 설정",
        "",
        "선택한 도표의 축·곡선·표시를 조정합니다.",
        "",
        Plot
    ),
    (
        InsertSolveBlock,
        "작성",
        "풀이 구간",
        "",
        "제약과 미지수를 함께 쓰는 풀이 구간을 만듭니다.",
        "",
        Always
    ),
    (
        InsertArea,
        "작성",
        "영역 묶음",
        "",
        "접거나 펼칠 수 있는 Area를 삽입합니다.",
        "",
        Always
    ),
    (
        InsertPageBreak,
        "문서",
        "페이지 나누기",
        "",
        "현재 작성 위치를 기준으로 페이지를 나눕니다.",
        "",
        Always
    ),
    (
        Fraction,
        "연산자",
        "분수",
        "/",
        "선택한 부분식을 분자로 쓰거나 분수 입력 칸을 만듭니다.",
        "a / b",
        Always
    ),
    (
        Power,
        "연산자",
        "거듭제곱",
        "^",
        "밑과 지수를 가진 구조 수식을 만듭니다.",
        "x²",
        Always
    ),
    (
        Root,
        "연산자",
        "근호",
        "",
        "제곱근 입력 칸을 만듭니다.",
        "√x",
        Always
    ),
    (
        Absolute,
        "연산자",
        "절댓값 / 행렬식",
        "",
        "스칼라에는 절댓값, 행렬에는 행렬식 구조를 사용합니다.",
        "|x|",
        Always
    ),
    (
        Factorial,
        "연산자",
        "팩토리얼",
        "",
        "피연산자에 팩토리얼을 적용합니다.",
        "n!",
        Always
    ),
    (
        Percent,
        "연산자",
        "백분율",
        "",
        "백분율 연산자를 삽입합니다.",
        "25%",
        Always
    ),
    (
        Degree,
        "연산자",
        "도 단위",
        "",
        "각도를 도 단위로 입력합니다.",
        "30°",
        Always
    ),
    (
        Polar,
        "연산자",
        "극형식",
        "",
        "크기와 위상으로 복소수를 작성합니다.",
        "r∠θ",
        Always
    ),
    (
        Transpose,
        "행렬",
        "전치",
        "",
        "벡터나 행렬의 전치 연산자를 삽입합니다.",
        "Aᵀ",
        Always
    ),
    (
        Conjugate,
        "연산자",
        "복소수 켤레",
        "",
        "복소수 켤레 연산자를 삽입합니다.",
        "",
        Always
    ),
    (
        Norm,
        "행렬",
        "노름",
        "",
        "벡터 또는 행렬의 노름 구조를 삽입합니다.",
        "‖A‖",
        Always
    ),
    (
        RowOperator,
        "행렬",
        "행 추출",
        "",
        "행렬에서 행을 추출하는 연산자를 삽입합니다.",
        "",
        Always
    ),
    (
        ColumnOperator,
        "행렬",
        "열 추출",
        "",
        "행렬에서 열을 추출하는 연산자를 삽입합니다.",
        "",
        Always
    ),
    (
        Vectorize,
        "행렬",
        "벡터화",
        "",
        "선택한 연산을 원소별 연산 구조로 만듭니다.",
        "",
        Always
    ),
    (
        CrossProduct,
        "행렬",
        "벡터 외적",
        "",
        "두 벡터의 외적 연산자를 삽입합니다.",
        "a × b",
        Always
    ),
    (
        Define,
        "연산자",
        "정의",
        " : ",
        "왼쪽 이름에 오른쪽 수식을 정의합니다.",
        "R := 25.6 Ω",
        Always
    ),
    (
        GlobalDefine,
        "연산자",
        "전역 정의",
        "",
        "워크시트 전역 정의 연산자를 삽입합니다.",
        "",
        Always
    ),
    (
        Evaluate,
        "계산",
        "결과 표시",
        "=",
        "현재 수식의 계산 결과를 표시합니다.",
        "R = 25.6 Ω",
        Always
    ),
    (
        SymbolicEvaluate,
        "계산",
        "기호 계산",
        "",
        "기호 계산 연산자를 삽입합니다.",
        "",
        Always
    ),
    (
        LiteralSubscript,
        "연산자",
        "이름 아래첨자",
        "Ctrl+−",
        "이름의 문자 아래첨자를 전환합니다. 확대·축소 키가 아닙니다.",
        "Vout → Vₒᵤₜ",
        Identifier
    ),
    (
        Matrix,
        "행렬",
        "행렬 삽입",
        "",
        "행과 열을 가진 구조 행렬을 삽입합니다.",
        "",
        Always
    ),
    (
        Index,
        "행렬",
        "배열 인덱스",
        "",
        "배열 원소를 참조하는 인덱스 칸을 만듭니다. 이름 아래첨자와 구분됩니다.",
        "vᵢ",
        Always
    ),
    (
        Range,
        "연산자",
        "범위 변수",
        "",
        "시작·간격·끝으로 범위를 정의합니다.",
        "i := 0..10",
        Always
    ),
    (
        Function,
        "함수",
        "함수 정의",
        "",
        "이름과 매개변수를 가진 함수 구조를 삽입합니다.",
        "f(x) := x²",
        Always
    ),
    (
        Program,
        "프로그램",
        "프로그램",
        "",
        "여러 줄로 구성된 수식 프로그램을 만듭니다.",
        "",
        Always
    ),
    (
        LocalDefine,
        "프로그램",
        "지역 대입",
        "",
        "프로그램 안에서 지역 변수에 대입합니다.",
        "",
        Always
    ),
    (
        If,
        "프로그램",
        "if 조건",
        "",
        "조건부 프로그램 줄을 삽입합니다.",
        "",
        Always
    ),
    (
        ElseIf,
        "프로그램",
        "else if",
        "",
        "추가 조건 분기를 삽입합니다.",
        "",
        Always
    ),
    (
        AlsoIf,
        "프로그램",
        "also if",
        "",
        "독립적인 추가 조건을 삽입합니다.",
        "",
        Always
    ),
    (
        Else,
        "프로그램",
        "else",
        "",
        "나머지 경우의 분기를 삽입합니다.",
        "",
        Always
    ),
    (
        ForLoop,
        "프로그램",
        "for 반복",
        "",
        "범위에 따라 반복하는 줄을 삽입합니다.",
        "",
        Always
    ),
    (
        WhileLoop,
        "프로그램",
        "while 반복",
        "",
        "조건에 따라 반복하는 줄을 삽입합니다.",
        "",
        Always
    ),
    (
        Return,
        "프로그램",
        "return",
        "",
        "프로그램의 반환 줄을 삽입합니다.",
        "",
        Always
    ),
    (
        Break,
        "프로그램",
        "break",
        "",
        "반복 종료 줄을 삽입합니다.",
        "",
        Always
    ),
    (
        Continue,
        "프로그램",
        "continue",
        "",
        "다음 반복으로 이동하는 줄을 삽입합니다.",
        "",
        Always
    ),
    (
        Try,
        "프로그램",
        "try",
        "",
        "오류 처리 구조를 삽입합니다.",
        "",
        Always
    ),
    (
        Integral,
        "미적분",
        "정적분",
        "",
        "적분 구간·함수·변수를 가진 구조를 삽입합니다.",
        "∫ f(x) dx",
        Always
    ),
    (
        Derivative,
        "미적분",
        "미분",
        "",
        "함수와 변수를 가진 미분 구조를 삽입합니다.",
        "d/dx f(x)",
        Always
    ),
    (
        PartialDerivative,
        "미적분",
        "편미분",
        "",
        "편미분 구조를 삽입합니다.",
        "∂/∂x f(x)",
        Always
    ),
    (
        Summation,
        "미적분",
        "합",
        "",
        "범위에 대한 합을 작성합니다.",
        "Σ",
        Always
    ),
    (
        Product,
        "미적분",
        "곱",
        "",
        "범위에 대한 곱을 작성합니다.",
        "Π",
        Always
    ),
    (
        InsertMatrixRow,
        "행렬",
        "행 삽입",
        "",
        "현재 셀 위치에 행을 삽입합니다.",
        "",
        Matrix
    ),
    (
        InsertMatrixColumn,
        "행렬",
        "열 삽입",
        "",
        "현재 셀 위치에 열을 삽입합니다.",
        "",
        Matrix
    ),
    (
        DeleteMatrixRow,
        "행렬",
        "행 삭제",
        "",
        "현재 셀의 행을 삭제합니다.",
        "",
        Matrix
    ),
    (
        DeleteMatrixColumn,
        "행렬",
        "열 삭제",
        "",
        "현재 셀의 열을 삭제합니다.",
        "",
        Matrix
    ),
    (
        ClearCells,
        "행렬",
        "셀 내용 지우기",
        "",
        "선택한 셀의 값을 지웁니다.",
        "",
        Matrix
    ),
    (
        CalculateNow,
        "계산",
        "지금 계산",
        "F5",
        "현재 보이는 활성 영역을 계산합니다.",
        "",
        Always
    ),
    (
        Recalculate,
        "계산",
        "워크시트 다시 계산",
        "Ctrl+F5",
        "워크시트 전체를 다시 계산합니다.",
        "",
        Always
    ),
    (
        StopAllCalculations,
        "계산",
        "계산 중지",
        "",
        "실행 중인 계산을 중지합니다.",
        "",
        Running
    ),
    (
        PreviousError,
        "계산",
        "이전 오류",
        "",
        "이전 계산 오류 영역으로 이동합니다.",
        "",
        Errors
    ),
    (
        NextError,
        "계산",
        "다음 오류",
        "",
        "다음 계산 오류 영역으로 이동합니다.",
        "",
        Errors
    ),
    (
        ToggleGrid,
        "보기",
        "격자 표시",
        "",
        "배치 격자를 표시하거나 숨깁니다.",
        "",
        Always
    ),
    (
        ToggleWarnings,
        "보기",
        "경고 표시",
        "",
        "재정의 경고의 표시를 전환합니다.",
        "",
        Always
    ),
];
pub(crate) fn find(action: A) -> Option<&'static Command> {
    COMMANDS.iter().find(|c| c.action == action)
}
pub(crate) fn tooltip(action: A, label: &str) -> String {
    find(action).map_or_else(
        || match action {
            A::InsertFunction(name) => format!(
                "{name}\n현재 수식 위치에 함수 호출을 삽입합니다. 인수 칸을 Tab으로 이동하세요."
            ),
            A::InsertUnit(name) => {
                format!("{name}\n단위 라벨을 가진 이름을 삽입합니다. 변수 이름과 구분됩니다.")
            }
            A::InsertSymbol(s) => format!("{s}\n현재 수식 위치에 기호를 삽입합니다."),
            _ => format!("{label}\n현재 선택과 편집 위치에 적용합니다."),
        },
        Command::tooltip,
    )
}
pub(crate) const EDITING_HELP: &[(&str, &str, &str, &str)] = &[
    (
        "Enter",
        "수정 확정",
        "수식·표 셀",
        "변경을 확정합니다. 프로그램 안에서는 새 줄을 작성합니다.",
    ),
    (
        "Esc",
        "영역 선택으로 돌아가기",
        "수식 편집",
        "현재 편집에서 빠져나와 영역을 선택합니다.",
    ),
    (
        "Space",
        "선택 확장",
        "수식 편집",
        "현재 부분식에서 바깥쪽 부분식으로 선택을 넓힙니다.",
    ),
    (
        "Tab / Shift+Tab",
        "입력 칸 이동",
        "수식·행렬·표",
        "다음 또는 이전 구조 입력 칸으로 이동합니다.",
    ),
    (
        "Shift+← / →",
        "문자 선택",
        "이름·텍스트",
        "문자 범위를 선택합니다.",
    ),
    (
        "Shift+Enter / Space",
        "행 / 열 추가",
        "행렬 편집",
        "행렬 문맥의 기존 행·열 삽입 동작을 사용합니다.",
    ),
    (
        "F1",
        "단축키 도움말",
        "편집 화면",
        "명령·키·적용 문맥을 검색합니다.",
    ),
    (
        "Ctrl+F1",
        "도구 모음 접기 / 펼치기",
        "편집 화면",
        "기존 리본 최소화 키를 도구 모음에 그대로 적용합니다.",
    ),
];

pub(crate) fn text_matches(query: &str, fields: &[&str]) -> bool {
    query.split_whitespace().all(|word| {
        let word = word.to_lowercase().replace('−', "-");
        fields
            .iter()
            .any(|value| value.to_lowercase().replace('−', "-").contains(&word))
    })
}
