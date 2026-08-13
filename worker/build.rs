fn main() {
    topcoat::tailwind::BuildConfig::new()
        .input("src/theme.css")
        .cwd(".")
        .render()
        .expect("Tailwind stylesheet generation failed");
}
