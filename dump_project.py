import os

# Configuration
root_dir = "."
output_file = "project_dump.txt"

# Directories to exclude
exclude_dirs = {
    "node_modules", 
    ".git", 
    "dist", 
    "target", 
    "build", 
    ".vscode", 
    ".idea",
    "__pycache__",
    "coverage"
}

# Files to exclude
exclude_files = {
    "package-lock.json", 
    "yarn.lock", 
    "pnpm-lock.yaml", 
    "cargo.lock", 
    "project_dump.txt", 
    "dump_project.py",
    ".DS_Store",
    "Thumbs.db",
    "sugarland_implementation_plan.md"
}

# Allowed extensions
allowed_extensions = {
    ".js", ".jsx", ".ts", ".tsx", ".html", ".css", ".scss", ".json", 
    ".rs", ".toml", ".py", ".md", ".txt", ".bat", ".sh", ".yml", ".yaml", 
    ".xml", ".sql", ".ini", ".conf", ".cfg"
}

def is_text_file(filename):
    _, ext = os.path.splitext(filename)
    return ext.lower() in allowed_extensions

def write_file_content(outfile, file_path, relative_path):
    try:
        with open(file_path, "r", encoding="utf-8") as infile:
            content = infile.read()
            outfile.write(f"================================================================================\n")
            outfile.write(f"File: {relative_path}\n")
            outfile.write(f"================================================================================\n")
            outfile.write(content)
            outfile.write("\n\n")
            print(f"Processed: {relative_path}")
    except Exception as e:
        print(f"Skipping {relative_path}: {e}")

def dump_files():
    # Write to dump file
    with open(output_file, "w", encoding="utf-8") as outfile:
        # Walk through directories
        for root, dirs, files in os.walk(root_dir):
            # Modify dirs in-place to skip excluded directories
            # We iterate safely by copying dirs list or just handling excludes
            # os.walk processes dirs list for next step.
            dirs[:] = [d for d in dirs if d not in exclude_dirs]
            
            for file in files:
                if file in exclude_files:
                    continue
                
                if not is_text_file(file):
                    continue
                
                file_path = os.path.join(root, file)
                # Normalize path separators
                relative_path = os.path.relpath(file_path, root_dir).replace("\\", "/")
                
                if relative_path == "dump_project.py" or relative_path == output_file:
                    continue

                write_file_content(outfile, file_path, relative_path)

if __name__ == "__main__":
    dump_files()
    print(f"Done. Project dumped to {output_file}")
