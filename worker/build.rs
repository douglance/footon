fn main() {
    topcoat::tailwind::BuildConfig::new()
        .input("styles.css")
        .cwd(".")
        .render()
        .expect("Tailwind stylesheet generation failed");
}
