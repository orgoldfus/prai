use ratatui::layout::{Alignment, Constraint, Flex, Layout};
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

use super::theme;

const TITLE: &str = r#"
 ██████╗ ██████╗  █████╗ ██╗
 ██╔══██╗██╔══██╗██╔══██╗██║
 ██████╔╝██████╔╝███████║██║
 ██╔═══╝ ██╔══██╗██╔══██║██║
 ██║     ██║  ██║██║  ██║██║
 ╚═╝     ╚═╝  ╚═╝╚═╝  ╚═╝╚═╝
"#;

const SUBTITLE: &str = "AI-Powered Code Review Assistant";

const HANDS: &str = r#"
                    -*#*-
                 -+:-=%@#=@%+
                 +=-:-=%@#-@%+-
                 :-:+%=+#@@=#@#-
                .=:-=%@-+#@@%++#%+:
                .=::==@@-=++@#=#@#-
                 +:.-+#%@:=+@@%-*@@=:
                 +-:::+#@@:=+@@@-+@@*-
                 --..::+*%%:=+%@@=+#@%=    =#@-
                .=::=@+:++%#:=+%@@++#%%+=+:+*%*
                .=:-*#@--+#@%:++%@%#=#@%+-:=+@@=
                 =-:-##%:-+%@%:+*%%@*=#@@*:-=@%-:
                 =-:-=#%#:=+%@#:=*%@@+=#@@#--#@@+-
                .=-::-+#%=:=+#%%-=*%@@+-#%@#=-+%%*-
                 +::.--+#@+:=+#@%:=#%@%==#@@%*-=##+-
                 =--:.--+%@*:=*%@%:=#%@@+=#@%@++-+#*=
                 :=-:.:--+%@*:=#@@#:+%%@@%+%%*@*#+=#*=-
                 .+:-:.--=+#%+:+#@@#:+%#@@@%%@+%*#*-+#+-
                  ==-:..--=+#@--+#@@%-#%+@@@@%@%##%#+==*=-
                  .+:=:.:--=#@%:=*#%@#+%%+@%%@%@%#%%##*#+--
                   ==:-:.---+#@+-+###%#%%#+%#@@%%##%%##+*-:
                   .+:-:..+:-+##=+###+#%%@%+%@@@##*%%##*+=:
                    -=:-..-=:=+#+=+###+*%%@@#%%@@%#*#%#*+=:
                     =-:-:.+--=*#*=##*#+*#%%@%%@@%%*###*+=:
                     .+:-:.:=:-=+#*=##+#%##%%%%%%%%*###*+=-
                     .+--::.=-:-=++=+##+++%%%%%%%#%#*###=+-
                      =+:-:.:+-:--==-=#*+++##@%%@%#%#+##++=-
                       ==:-:::+::-=====##++=+*%@@@@++*#+++*=--*#%%%#*   ...
                       .-=:-:.:+::-=====+%*+===+#@@@%=+*+=*+#@@@@@@@@+*#+=:..
                         -+:-::.+-::-=+===+##++==+#%@@%*#+%@@@@@@@@*+=====+=:..
                          :+--::.-=::--====+=+#%#+++##*=#@@@@@@@@==--==++++++=-....
                           .+-:::.:=-::--==-==+++*#*===@@@@@@@@+-:..-===----------:.....
                            .==:::..:=-:::-==-::-----+@@@@@%@@-.....==-:::..:-++++++=---....
                             .:+-:::..:--:::--===---%@%*@@#@*:.....:-:.:..:-++++++======:....
                               --=::::....--::::---%#@+@@%#=.......:.....-=======-===-...
                              -*..:-:::.......::::+*#%*@%#-.............:------::--:..
                              ==:................:#+=+*##-.............:---...:--:...
                             :#-=......::...:....+#+=**#..............:-:...:--:...
                             =#-=-......::::....-##+=*#..............::....--:....
                             ++===:......-......=##++#-..................:--..... 
                             *==+-:....=-.......=##++-..................:::.....
                             *=+=-:..-+:........:+##=..................::.......
                             +#*---**:............-*...........................
                             =@%%%=...........................................
                              ::-=-:..........................................
                                .....................:........................
                                     .................   ....................
                                            ..........      .................
                                                 .....           ...........
                                                                     .......
                                                                         .
"#;

/// Render the splash screen with title on the left and praying hands on the right.
pub fn render(frame: &mut Frame) {
    let area = frame.area();

    frame.render_widget(Clear, area);
    frame.render_widget(Block::default().style(theme::text()), area);

    // Title + subtitle
    let mut left_lines: Vec<Line<'_>> = TITLE
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| Line::from(l).style(theme::accent()))
        .collect();
    left_lines.push(Line::from(""));
    left_lines.push(Line::from(SUBTITLE).style(theme::accent()));
    let left_height = left_lines.len() as u16;

    // Hands art — strip common leading indent and trailing whitespace
    let non_empty: Vec<&str> = HANDS.lines().filter(|l| !l.trim().is_empty()).collect();
    let min_indent = non_empty
        .iter()
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let hands_lines: Vec<Line<'_>> = non_empty
        .into_iter()
        .map(|l| {
            let stripped = if l.len() > min_indent {
                &l[min_indent..]
            } else {
                l
            };
            Line::from(stripped.trim_end()).style(theme::accent())
        })
        .collect();
    let hands_height = hands_lines.len() as u16;

    let total_height = hands_height.max(left_height);

    // Centre the whole splash vertically
    let vert = Layout::vertical([Constraint::Length(total_height)])
        .flex(Flex::Center)
        .split(area);

    // Split into left (title) and right (hands)
    let horiz =
        Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)]).split(vert[0]);

    // Title + subtitle, vertically centred
    let left_vert = Layout::vertical([Constraint::Length(left_height)])
        .flex(Flex::Center)
        .split(horiz[0]);
    frame.render_widget(
        Paragraph::new(Text::from(left_lines))
            .alignment(Alignment::Center)
            .block(Block::default()),
        left_vert[0],
    );

    // Hands, vertically centred
    let right_vert = Layout::vertical([Constraint::Length(hands_height)])
        .flex(Flex::Center)
        .split(horiz[1]);
    frame.render_widget(
        Paragraph::new(Text::from(hands_lines)).block(Block::default()),
        right_vert[0],
    );
}
