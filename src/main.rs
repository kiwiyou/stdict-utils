use anyhow::{Context, Result, anyhow, bail};
use quick_xml::{Reader as XmlReader, XmlVersion, events::Event as XmlEvent};
use std::io::BufRead;

fn main() -> Result<()> {
    for read_dir in std::fs::read_dir("vendor/stdict").context("Failed to read stdict directory")? {
        let dir_entry = read_dir.context("Failed to read directory content")?;
        let path = dir_entry.path();
        let mut reader = XmlReader::from_file(&path).with_context(|| {
            format!(
                "Failed to initiate xml reader for {}",
                path.to_string_lossy()
            )
        })?;
        let words = read_root(&mut reader).with_context(|| {
            format!("Failed to load stdict data from {}", path.to_string_lossy())
        })?;
    }
    Ok(())
}

fn read_root<R: BufRead>(reader: &mut XmlReader<R>) -> Result<Vec<Item>> {
    let mut buf = vec![];
    let mut items = vec![];
    let mut xml_version = XmlVersion::Implicit1_0;
    loop {
        let event = reader
            .read_event_into(&mut buf)
            .context("Failed to parse root structure")?;
        match event {
            quick_xml::events::Event::Eof => break,
            quick_xml::events::Event::Decl(decl) => {
                xml_version = decl
                    .xml_version()
                    .context("Failed to recognize XML version")?;
            }
            quick_xml::events::Event::Start(start) => {
                let name = start.name();
                if name.0 == b"item" {
                    items.push(Item::read(reader, xml_version)?);
                }
            }
            _ => {}
        }
    }
    Ok(items)
}

macro_rules! label_or_name {
    ($label:literal $name:ident) => {
        $label
    };
    ($name:ident) => {
        stringify!($name)
    };
}

macro_rules! read_scalar {
    (@unique $reader:ident, $xml_version:ident, $buf:ident, $name:ident$(: $parse_as:ty)?$(=$label:literal)?) => {
        $name = match $name {
            Some(existing) => bail!(concat!("Found duplicated `", label_or_name!($($label)? $name), "`. Previous was: {:?}"), existing),
            None => Some(read_scalar!($reader, $xml_version, $buf, $name$(: $parse_as)?$(=$label)?)),
        }
    };
    ($reader:ident, $xml_version:ident, $buf:ident, $name:ident$(: $parse_as:ty)?$(=$label:literal)?) => {
        read_leaf($reader, $xml_version, &mut $buf, label_or_name!($($label)? $name).as_bytes())
            .context(concat!("Failed to read `", stringify!($name), "`"))?
            $(
                .parse::<$parse_as>()
                .context(concat!("Failed to parse `", stringify!($name), "`"))?
            )?
    };
}

/// 표제어 항목
#[derive(Debug, Clone)]
struct Item {
    /// 표제어 ID
    ///
    /// 예: `486`
    target_code: usize,
    /// 표제어
    ///
    /// 예: `가지`
    word: String,
    /// 표제어 단위
    ///
    /// 구, 관용구, 단어, 속담.
    ///
    /// 예: `관용구`
    word_unit: String,
    /// 어원 분류
    ///
    /// 속담인 경우에는 존재하지 않습니다.
    ///
    /// 고유어, 한자어, 혼종어, 외래어.
    ///
    /// 예: `혼종어`
    word_type: Option<String>,
    /// 원어 정보
    ///
    /// 표제어 내에서 원어와 대응되는 부분끼리 순서대로 주어집니다.
    original_language_info: Vec<OriginalLanguageInfo>,
    /// 허용 발음
    pronunciation_info: Vec<PronunciationInfo>,
    /// 활용형
    conjugation_info: Vec<ConjugationInfo>,
    /// 관련 어휘 정보
    relation_info: Vec<RelationInfo>,
    /// 어원
    ///
    /// 예: `크다＜용가＞`
    origin: Option<String>,
    /// 이형태
    ///
    /// 예: `U자형 배수관`
    allomorph: Vec<String>,
    /// 표제어 단위로 적용되는 어휘 관계 정보
    lexical_info: Vec<LexicalInfo>,
    /// 품사별 정보
    ///
    /// 다의어는 여러 품사를 가질 수 있습니다.
    pos_info: Vec<PosInfo>,
}

impl Item {
    const NAME: &[u8] = b"item";

    fn read<R: BufRead>(reader: &mut XmlReader<R>, xml_version: XmlVersion) -> Result<Self> {
        let mut buf = vec![];
        let mut target_code = None;
        let mut word = None;
        let mut word_unit = None;
        let mut word_type = None;
        let mut original_language_info = vec![];
        let mut relation_info = vec![];
        let mut pronunciation_info = vec![];
        let mut conjugation_info = vec![];
        let mut origin = None;
        let mut allomorph = vec![];
        let mut lexical_info = vec![];
        let mut pos_info = vec![];
        loop {
            match reader.read_event_into(&mut buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"target_code" => {
                        read_scalar!(@unique reader, xml_version, buf, target_code: usize)
                    }
                    b"word" => read_scalar!(@unique reader, xml_version, buf, word),
                    b"word_unit" => read_scalar!(@unique reader, xml_version, buf, word_unit),
                    b"word_type" => read_scalar!(@unique reader, xml_version, buf, word_type),
                    OriginalLanguageInfo::NAME => original_language_info.push(
                        OriginalLanguageInfo::read(reader, xml_version, &mut buf)
                            .context("Failed to read `original_language_info`")?,
                    ),
                    PronunciationInfo::NAME => pronunciation_info.push(
                        PronunciationInfo::read(reader, xml_version, &mut buf)
                            .context("Failed to read `pronunciation_info`")?,
                    ),
                    RelationInfo::NAME => relation_info.push(
                        RelationInfo::read(reader, xml_version, &mut buf)
                            .context("Failed to read `relation_info`")?,
                    ),
                    ConjugationInfo::NAME => conjugation_info.push(
                        ConjugationInfo::read(reader, xml_version, &mut buf)
                            .context("Failed to read `conjugation_info`")?,
                    ),
                    b"origin" => read_scalar!(@unique reader, xml_version, buf, origin),
                    b"allomorph" => {
                        allomorph.push(read_scalar!(reader, xml_version, buf, allomorph))
                    }
                    LexicalInfo::NAME => lexical_info.push(
                        LexicalInfo::read(reader, xml_version, &mut buf)
                            .context("Failed to read `lexical_info`")?,
                    ),
                    PosInfo::NAME => pos_info.push(
                        PosInfo::read(reader, xml_version, &mut buf)
                            .context("Failed to read `pos_info`")?,
                    ),
                    _ => {}
                },
                _ => {}
            }
        }
        Ok(Self {
            target_code: target_code.ok_or_else(|| anyhow!("`target_code` not found"))?,
            word: word.ok_or_else(|| anyhow!("`word` not found"))?,
            word_unit: word_unit.ok_or_else(|| anyhow!("`word_unit` not found"))?,
            word_type,
            original_language_info,
            pronunciation_info,
            conjugation_info,
            origin,
            allomorph,
            relation_info,
            lexical_info,
            pos_info,
        })
    }
}

// 원어 정보
#[derive(Debug, Clone)]
struct OriginalLanguageInfo {
    /// 원어
    original_language: String,
    /// 원어 구분
    ///
    /// 말레이어, 세르보·크로아트어, /(병기), 프랑스어, 헝가리어, 러시아어,
    /// 몽골어, 네덜란드어, 한자, 안 밝힘, 루마니아어, 이탈리아어, 타이어,
    /// 고유어, 영어, 히브리어, 스웨덴어, 라틴어, 일본어, 아랍어, 체코어,
    /// 포르투갈어, 인도네시아어, 노르웨이어, 힌디어, 기타어, 불가리아어,
    /// 독일어, 페르시아어, 터키어, 에스파냐어, 그리스어, 산스크리트어,
    /// 핀란드어, 중국어, 베트남어.
    ///
    /// 예: `영어`
    language_type: String,
}

impl OriginalLanguageInfo {
    const NAME: &[u8] = b"original_language_info";

    fn read<R: BufRead>(
        reader: &mut XmlReader<R>,
        xml_version: XmlVersion,
        mut buf: &mut Vec<u8>,
    ) -> Result<Self> {
        let mut original_language = None;
        let mut language_type = None;
        loop {
            match reader.read_event_into(buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"original_language" => {
                        read_scalar!(@unique reader, xml_version, buf, original_language)
                    }
                    b"language_type" => {
                        read_scalar!(@unique reader, xml_version, buf, language_type)
                    }
                    _ => {}
                },
                _ => {}
            };
        }
        Ok(Self {
            original_language: original_language
                .ok_or_else(|| anyhow!("`original_language` not found"))?,
            language_type: language_type.ok_or_else(|| anyhow!("`language_type` not found"))?,
        })
    }
}

/// 발음 정보
#[derive(Debug, Clone)]
struct PronunciationInfo {
    /// 발음
    ///
    /// 예: `피ː고용인`
    pronunciation: String,
}

impl PronunciationInfo {
    const NAME: &[u8] = b"pronunciation_info";

    fn read<R: BufRead>(
        reader: &mut XmlReader<R>,
        xml_version: XmlVersion,
        mut buf: &mut Vec<u8>,
    ) -> Result<Self> {
        let mut pronunciation = None;
        loop {
            match reader.read_event_into(buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"pronunciation" => {
                        read_scalar!(@unique reader, xml_version, buf, pronunciation)
                    }
                    _ => {}
                },
                _ => {}
            };
        }
        Ok(Self {
            pronunciation: pronunciation.ok_or_else(|| anyhow!("`pronunciation` not found"))?,
        })
    }
}

/// 활용 정보
#[derive(Debug, Clone)]
struct ConjugationInfo {
    /// 활용형
    ///
    /// 예: `각봉하여`
    conjugation: String,
    /// 활용형의 발음
    ///
    /// 예: `각뽕하여`
    conjugation_pronunciation_info: Vec<PronunciationInfo>,
    /// 활용형의 준말
    ///
    /// 예: `각뽕해`
    abbreviation_info: Option<AbbreviationInfo>,
}

impl ConjugationInfo {
    const NAME: &[u8] = b"conju_info";

    fn read<R: BufRead>(
        reader: &mut XmlReader<R>,
        xml_version: XmlVersion,
        mut buf: &mut Vec<u8>,
    ) -> Result<Self> {
        let mut conjugation = None;
        let mut conjugation_pronunciation_info = vec![];
        let mut abbreviation_info = None;
        loop {
            match reader.read_event_into(buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"conjugation" => read_scalar!(@unique reader, xml_version, buf, conjugation),
                    PronunciationInfo::NAME => conjugation_pronunciation_info.push(
                        PronunciationInfo::read(reader, xml_version, buf)
                            .context("Failed to read `pronunciation_info`")?,
                    ),
                    AbbreviationInfo::NAME => {
                        abbreviation_info = Some(match abbreviation_info {
                            Some(existing) => bail!(
                                "Found duplicated `abbreviation_info`. Previous was: {existing:?}"
                            ),
                            None => AbbreviationInfo::read(reader, xml_version, buf)
                                .context("Failed to read `abbreviation_info`")?,
                        })
                    }
                    _ => {}
                },
                _ => {}
            };
        }
        Ok(Self {
            conjugation: conjugation.ok_or_else(|| anyhow!("`conjugation` not found"))?,
            conjugation_pronunciation_info,
            abbreviation_info,
        })
    }
}

/// 활용형의 준말 정보
#[derive(Debug, Clone)]
struct AbbreviationInfo {
    /// 준말
    ///
    /// 예: `각봉해`
    abbreviation: String,
    /// 준말의 발음
    ///
    /// 예: `각뽕해`
    pronunciation_info: Vec<PronunciationInfo>,
}

impl AbbreviationInfo {
    const NAME: &[u8] = b"abbreviation_info";

    fn read<R: BufRead>(
        reader: &mut XmlReader<R>,
        xml_version: XmlVersion,
        mut buf: &mut Vec<u8>,
    ) -> Result<Self> {
        let mut abbreviation = None;
        let mut pronunciation_info = vec![];
        loop {
            match reader.read_event_into(buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"abbreviation" => read_scalar!(@unique reader, xml_version, buf, abbreviation),
                    PronunciationInfo::NAME => pronunciation_info.push(
                        PronunciationInfo::read(reader, xml_version, buf)
                            .context("Failed to read `pronunciation_info`")?,
                    ),
                    _ => {}
                },
                _ => {}
            };
        }
        Ok(Self {
            abbreviation: abbreviation.ok_or_else(|| anyhow!("`abbreviation` not found"))?,
            pronunciation_info,
        })
    }
}

/// 어휘 관계 정보
///
/// 유의어, 준말 등으로 넘겨주기 위한 항목.
#[derive(Debug, Clone)]
struct LexicalInfo {
    /// 참고 표제어
    ///
    /// 예: 가지
    word: String,
    /// 참고 정보가 표시될 위치
    ///
    /// - `어휘`인 경우 표제어([`Item`]) 단위로 적용되는 정보.
    /// - `품사`인 경우 특정 품사별 정보([`PosInfo`]) 단위로 적용되는 정보.
    /// - `의미`인 경우 특정 의미([`SenseInfo`]) 단위로 적용되는 정보.
    ///
    /// "반짝거리다" 항목이 좋은 예시입니다.
    unit: String,
    /// 참고 분류
    ///
    /// 비슷한말, 준말, 본말.
    ///
    /// 예: `비슷한말`
    r#type: String,
    /// 넘겨받을 항목의 코드.
    ///
    /// - [`Self::unit`]이 `어휘`인 경우 [`Item::target_code`].
    /// - [`Self::unit`]이 `품사`인 경우 [`PosInfo::pos_code`].
    /// - [`Self::unit`]이 `의미`인 경우 [`SenseInfo::sense_code`].
    link_target_code: String,
    /// 넘겨받을 항목의 표준국어대사전상 링크.
    ///
    /// 예: `https://stdict.korean.go.kr/search/searchView.do?word_no=131104&searchKeywordTo=3`
    link: String,
}

impl LexicalInfo {
    const NAME: &[u8] = b"lexical_info";

    fn read<R: BufRead>(
        reader: &mut XmlReader<R>,
        xml_version: XmlVersion,
        mut buf: &mut Vec<u8>,
    ) -> Result<Self> {
        let mut word = None;
        let mut unit = None;
        let mut r#type = None;
        let mut link_target_code = None;
        let mut link = None;
        loop {
            match reader.read_event_into(buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"word" => read_scalar!(@unique reader, xml_version, buf, word),
                    b"unit" => read_scalar!(@unique reader, xml_version, buf, unit),
                    b"type" => read_scalar!(@unique reader, xml_version, buf, r#type = "type"),
                    b"link_target_code" => {
                        read_scalar!(@unique reader, xml_version, buf, link_target_code)
                    }
                    b"link" => read_scalar!(@unique reader, xml_version, buf, link),
                    _ => {}
                },
                _ => {}
            };
        }
        Ok(Self {
            word: word.ok_or_else(|| anyhow!("`word` not found"))?,
            unit: unit.ok_or_else(|| anyhow!("`unit` not found"))?,
            r#type: r#type.ok_or_else(|| anyhow!("`type` not found"))?,
            link_target_code: link_target_code
                .ok_or_else(|| anyhow!("`link_target_code` not found"))?,
            link: link.ok_or_else(|| anyhow!("`link` not found"))?,
        })
    }
}

/// 연관 정보
///
/// 부표제어, 관용구, 속담으로 넘겨주기 위한 항목.
#[derive(Debug, Clone)]
struct RelationInfo {
    /// 참고 표제어
    ///
    /// 예: `가지`
    word: String,
    /// 참고 분류
    ///
    /// 부표제어, 관용구, 속담.
    ///
    /// 예: `부표제어`
    r#type: String,
    /// 넘겨받을 항목의 코드.
    ///
    /// [`Item::target_code`] 참조.
    link_target_code: String,
    /// 넘겨받을 항목의 표준국어대사전상 링크.
    ///
    /// 예: `https://stdict.korean.go.kr/search/searchView.do?word_no=360155&searchKeywordTo=1`
    link: String,
}

impl RelationInfo {
    const NAME: &[u8] = b"relation_info";

    fn read<R: BufRead>(
        reader: &mut XmlReader<R>,
        xml_version: XmlVersion,
        mut buf: &mut Vec<u8>,
    ) -> Result<Self> {
        let mut word = None;
        let mut r#type = None;
        let mut link_target_code = None;
        let mut link = None;
        loop {
            match reader.read_event_into(buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"word" => read_scalar!(@unique reader, xml_version, buf, word),
                    b"type" => read_scalar!(@unique reader, xml_version, buf, r#type = "type"),
                    b"link_target_code" => {
                        read_scalar!(@unique reader, xml_version, buf, link_target_code)
                    }
                    b"link" => read_scalar!(@unique reader, xml_version, buf, link),
                    _ => {}
                },
                _ => {}
            };
        }
        Ok(Self {
            word: word.ok_or_else(|| anyhow!("`word` not found"))?,
            r#type: r#type.ok_or_else(|| anyhow!("`type` not found"))?,
            link_target_code: link_target_code
                .ok_or_else(|| anyhow!("`link_target_code` not found"))?,
            link: link.ok_or_else(|| anyhow!("`link` not found"))?,
        })
    }
}

/// 품사별 정보
#[derive(Debug, Clone)]
struct PosInfo {
    /// 품사
    ///
    /// 접사, 의존 명사, 감탄사, 보조 동사, 조사, 구, 어미, 동사, 부사,
    /// 관형사, 형용사, 대명사, 명사, 수사, 품사 없음, 보조 형용사.
    ///
    /// 예: `관형사`
    pos: String,
    /// 품사별 정보 ID
    ///
    /// 예: `31395001`
    pos_code: usize,
    /// 품사별 정보 단위로 적용되는 어휘 관계 정보
    lexical_info: Vec<LexicalInfo>,
    /// 문형 형태가 유사한 항목별 정보
    common_pattern_info: Vec<CommonPatternInfo>,
}

impl PosInfo {
    const NAME: &[u8] = b"pos_info";

    fn read<R: BufRead>(
        reader: &mut XmlReader<R>,
        xml_version: XmlVersion,
        mut buf: &mut Vec<u8>,
    ) -> Result<Self> {
        let mut pos = None;
        let mut pos_code = None;
        let mut lexical_info = vec![];
        let mut common_pattern_info = vec![];
        loop {
            match reader.read_event_into(buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"pos" => read_scalar!(@unique reader, xml_version, buf, pos),
                    b"pos_code" => {
                        read_scalar!(@unique reader, xml_version, buf, pos_code: usize)
                    }
                    LexicalInfo::NAME => lexical_info.push(
                        LexicalInfo::read(reader, xml_version, buf)
                            .context("Failed to read `lexical_info`")?,
                    ),
                    CommonPatternInfo::NAME => common_pattern_info.push(
                        CommonPatternInfo::read(reader, xml_version, buf)
                            .context("Failed to read `common_pattern_info`")?,
                    ),
                    _ => {}
                },
                _ => {}
            };
        }
        Ok(Self {
            pos: pos.ok_or_else(|| anyhow!("`pos` not found"))?,
            pos_code: pos_code.ok_or_else(|| anyhow!("`pos_code` not found"))?,
            lexical_info,
            common_pattern_info,
        })
    }
}

/// 공통 문형 항목
#[derive(Debug, Clone)]
struct CommonPatternInfo {
    /// 공통 문형 코드
    ///
    /// 예: `5213001001`
    common_pattern_code: usize,
    /// 문형 정보
    pattern_info: Vec<PatternInfo>,
    /// 문법 정보
    grammar_info: Vec<GrammarInfo>,
    /// 공통 문형 단위로 적용되는 어휘 관계 정보
    lexical_info: Vec<LexicalInfo>,
    /// 의미
    sense_info: Vec<SenseInfo>,
}

impl CommonPatternInfo {
    const NAME: &[u8] = b"comm_pattern_info";

    fn read<R: BufRead>(
        reader: &mut XmlReader<R>,
        xml_version: XmlVersion,
        mut buf: &mut Vec<u8>,
    ) -> Result<Self> {
        let mut common_pattern_code = None;
        let mut pattern_info = vec![];
        let mut grammar_info = vec![];
        let mut lexical_info = vec![];
        let mut sense_info = vec![];
        loop {
            match reader.read_event_into(buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"comm_pattern_code" => {
                        read_scalar!(@unique reader, xml_version, buf, common_pattern_code: usize = "comm_pattern_code")
                    }
                    PatternInfo::NAME => pattern_info.push(
                        PatternInfo::read(reader, xml_version, buf)
                            .context("Failed to read `pattern_info`")?,
                    ),
                    GrammarInfo::NAME => grammar_info.push(
                        GrammarInfo::read(reader, xml_version, buf)
                            .context("Failed to read `grammar_info`")?,
                    ),
                    LexicalInfo::NAME => lexical_info.push(
                        LexicalInfo::read(reader, xml_version, buf)
                            .context("Failed to read `lexical_info`")?,
                    ),
                    SenseInfo::NAME => sense_info.push(
                        SenseInfo::read(reader, xml_version, buf)
                            .context("Failed to read `sense_info`")?,
                    ),
                    _ => {}
                },
                _ => {}
            };
        }
        Ok(Self {
            common_pattern_code: common_pattern_code
                .ok_or_else(|| anyhow!("`comm_pattern_code` not found"))?,
            pattern_info,
            grammar_info,
            lexical_info,
            sense_info,
        })
    }
}

/// 문형 정보
#[derive(Debug, Clone)]
struct PatternInfo {
    /// 구문 틀
    ///
    /// 예: `(…과)`
    pattern: String,
}

impl PatternInfo {
    const NAME: &[u8] = b"pattern_info";

    fn read<R: BufRead>(
        reader: &mut XmlReader<R>,
        xml_version: XmlVersion,
        mut buf: &mut Vec<u8>,
    ) -> Result<Self> {
        let mut pattern = None;
        loop {
            match reader.read_event_into(buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"pattern" => {
                        read_scalar!(@unique reader, xml_version, buf, pattern)
                    }
                    _ => {}
                },
                _ => {}
            };
        }
        Ok(Self {
            pattern: pattern.ok_or_else(|| anyhow!("`pattern` not found"))?,
        })
    }
}

/// 문법 정보
#[derive(Debug, Clone)]
struct GrammarInfo {
    /// 문법 주석
    ///
    /// 예: `‘…과’가 나타나지 않을 때는 여럿임을 뜻하는 말이 주어로 온다`
    grammar: String,
}

impl GrammarInfo {
    const NAME: &[u8] = b"grammar_info";

    fn read<R: BufRead>(
        reader: &mut XmlReader<R>,
        xml_version: XmlVersion,
        mut buf: &mut Vec<u8>,
    ) -> Result<Self> {
        let mut grammar = None;
        loop {
            match reader.read_event_into(buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"grammar" => {
                        read_scalar!(@unique reader, xml_version, buf, grammar)
                    }
                    _ => {}
                },
                _ => {}
            };
        }
        Ok(Self {
            grammar: grammar.ok_or_else(|| anyhow!("`grammar` not found"))?,
        })
    }
}

/// 의미 항목
#[derive(Debug, Clone)]
struct SenseInfo {
    /// 의미 코드
    ///
    /// 예: `334247``
    sense_code: usize,
    /// 의미 범주
    ///
    /// 우리말샘에서 일반어 / 지역어(방언) / 북한어 / 옛말로 구분하던 항목이었으나
    /// 2026년 기준 표준국어대사전에서는 일반어만 등재합니다.
    ///
    /// 예: `일반어``
    r#type: String,
    /// 뜻풀이
    ///
    /// 참조가 필요한 항목은 숫자가 달려 있습니다.
    ///
    /// 예: ```소리의 차이나 변수를 나타내기 위하여 덧붙이는 소문자. 문자의 좌우(左右)의
    /// 상하(上下)에 붙이는 것으로 ‘<I>X<sub style='font-size:11px;'>i</sub></I>’,
    /// ‘<I>X<sup style='font-size:11px;'>h</sup></I>’에 쓰인 <I>i, h</I>
    /// 따위이다.```
    definition: String,
    /// 뜻풀이 (링크 포함)
    ///
    /// 참조가 필요한 항목에 링크가 마크업으로 삽입되어 있습니다.
    ///
    /// 예: `범죄 집단의 은어로, ‘<sense_no>425721</sense_no>성냥’을 이르는 말.`
    definition_original: String,
    /// 학명
    ///
    /// 예: `Aconitum jaluense`
    scientific_name: Option<String>,
    /// 전문 분야
    ///
    /// 예: `전기·전자`
    category: Option<String>,
    /// 용례 정보
    example_info: Vec<ExampleInfo>,
    /// 멀티미디어 정보
    multimedia_info: Vec<MultimediaInfo>,
    /// 의미 단위에 적용되는 어휘 관계 정보
    lexical_info: Vec<LexicalInfo>,
}

impl SenseInfo {
    const NAME: &[u8] = b"sense_info";

    fn read<R: BufRead>(
        reader: &mut XmlReader<R>,
        xml_version: XmlVersion,
        mut buf: &mut Vec<u8>,
    ) -> Result<Self> {
        let mut sense_code = None;
        let mut r#type = None;
        let mut definition = None;
        let mut definition_original = None;
        let mut scientific_name = None;
        let mut category: Option<String> = None;
        let mut example_info = vec![];
        let mut multimedia_info = vec![];
        let mut lexical_info = vec![];
        loop {
            match reader.read_event_into(buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"sense_code" => {
                        read_scalar!(@unique reader, xml_version, buf, sense_code: usize)
                    }
                    b"type" => read_scalar!(@unique reader, xml_version, buf, r#type = "type"),
                    b"definition" => read_scalar!(@unique reader, xml_version, buf, definition),
                    b"definition_original" => {
                        read_scalar!(@unique reader, xml_version, buf, definition_original)
                    }
                    b"scientific_name" => {
                        read_scalar!(@unique reader, xml_version, buf, scientific_name)
                    }
                    b"cat" => {
                        let incoming = read_scalar!(reader, xml_version, buf, category = "cat");
                        if let Some(existing) = &category
                            && incoming != "없음"
                        {
                            bail!("Found duplicated `category`. Previous was: {existing:?}");
                        } else {
                            category = Some(incoming);
                        }
                    }
                    ExampleInfo::NAME => {
                        let example = ExampleInfo::read(reader, xml_version, buf)
                            .context("Failed to read `example_info`")?;
                        if let Some(example) = example {
                            example_info.push(example);
                        }
                    }
                    MultimediaInfo::NAME => multimedia_info.push(
                        MultimediaInfo::read(reader, xml_version, buf)
                            .context("Failed to read `multimedia_info`")?,
                    ),
                    LexicalInfo::NAME => lexical_info.push(
                        LexicalInfo::read(reader, xml_version, buf)
                            .context("Failed to read `lexical_info`")?,
                    ),
                    _ => {}
                },
                _ => {}
            };
        }
        Ok(Self {
            sense_code: sense_code.ok_or_else(|| anyhow!("`sense_code` not found"))?,
            r#type: r#type.ok_or_else(|| anyhow!("`r#type` not found"))?,
            definition: definition.ok_or_else(|| anyhow!("`definition` not found"))?,
            definition_original: definition_original
                .ok_or_else(|| anyhow!("`definition_original` not found"))?,
            scientific_name,
            category,
            example_info,
            multimedia_info,
            lexical_info,
        })
    }
}

/// 용례 정보
#[derive(Debug, Clone)]
struct ExampleInfo {
    /// 용례 문장
    ///
    /// 예: `전화도 회선이 모자라기 때문에 기다리라고 차일피일 미루기만 할 뿐 놓아 줄 눈치가 보이지 않고….`
    example: String,
    /// 출전
    ///
    /// 예: `안정효, 하얀 전쟁`
    source: Option<String>,
}

impl ExampleInfo {
    const NAME: &[u8] = b"example_info";

    // <example_info></example_info> 형태인 경우를 무시해야 함
    fn read<R: BufRead>(
        reader: &mut XmlReader<R>,
        xml_version: XmlVersion,
        mut buf: &mut Vec<u8>,
    ) -> Result<Option<Self>> {
        let mut example = None;
        let mut source = None;
        loop {
            match reader.read_event_into(buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"example" => read_scalar!(@unique reader, xml_version, buf, example),
                    b"source" => read_scalar!(@unique reader, xml_version, buf, source),
                    _ => {}
                },
                _ => {}
            };
        }
        Ok(example.map(|example| Self { example, source }))
    }
}

/// 미디어 정보
#[derive(Debug, Clone)]
struct MultimediaInfo {
    /// 자료 제목
    ///
    /// 예: `번호판`
    label: Option<String>,
    /// 미디어 형식 분류
    ///
    /// 삽화, 사진, 동영상, 애니메이션.
    ///
    /// 예: `사진`
    r#type: String,
    /// 자료 URL
    ///
    /// 예: `http://media.korean.go.kr/front/view/mediaView.do?file_no=198238`
    link: String,
}

impl MultimediaInfo {
    const NAME: &[u8] = b"multimedia_info";

    fn read<R: BufRead>(
        reader: &mut XmlReader<R>,
        xml_version: XmlVersion,
        mut buf: &mut Vec<u8>,
    ) -> Result<Self> {
        let mut label = None;
        let mut r#type = None;
        let mut link = None;
        loop {
            match reader.read_event_into(buf)? {
                XmlEvent::End(end) => {
                    if end.name().0 == Self::NAME {
                        break;
                    }
                }
                XmlEvent::Start(start) => match start.name().0 {
                    b"label" => {
                        read_scalar!(@unique reader, xml_version, buf, label)
                    }
                    b"type" => {
                        read_scalar!(@unique reader, xml_version, buf, r#type = "type")
                    }
                    b"link" => {
                        read_scalar!(@unique reader, xml_version, buf, link)
                    }
                    _ => {}
                },
                _ => {}
            };
        }
        Ok(Self {
            label,
            r#type: r#type.ok_or_else(|| anyhow!("`r#type` not found"))?,
            link: link.ok_or_else(|| anyhow!("`link` not found"))?,
        })
    }
}

fn read_leaf<R: BufRead>(
    reader: &mut XmlReader<R>,
    xml_version: XmlVersion,
    buf: &mut Vec<u8>,
    end: &[u8],
) -> Result<String> {
    let mut text = String::new();
    loop {
        let content = match reader.read_event_into(buf)? {
            XmlEvent::Comment(_) => continue,
            XmlEvent::End(bytes_end) if bytes_end.name().0 == end => break Ok(text),
            XmlEvent::Text(bytes_text) => bytes_text.xml_content(xml_version)?,
            XmlEvent::CData(bytes_cdata) => bytes_cdata.xml_content(xml_version)?,
            XmlEvent::GeneralRef(bytes_ref) => bytes_ref.xml_content(xml_version)?,
            e => bail!("Unexpected xml event: {e:?}"),
        };
        text.push_str(&content);
    }
}
