export default {
	extends: ["@commitlint/config-conventional"],
	rules: {
		"header-max-length": [2, "always", 100],
		"type-enum": [
			2,
			"always",
			["feat", "fix", "refactor", "docs", "style", "chore", "test"]
		],
		"subject-case": [2, "always", ["lower-case"]],
		"subject-full-stop": [2, "never", "."]
	}
};
