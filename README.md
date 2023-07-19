# Using this in Python

0. [Make sure Cargo is installed](https://doc.rust-lang.org/cargo/getting-started/installation.html)
1. Create and activate a VENV
2. `pip install maturin`
3. `maturin develop` or `maturin build`, whatever works

# Python Functions
* `search_acronym(search, category_name)`: Searches the acronym dictionary.
* `generate_training_data(num_samples, output_path)`: Finds a bunch of random acronym definitions + examples and adds them to a CSV file. I would highly recommend loading more than you think you'll need, because a lot won't have enough definitions to be formatted for MCQ
* `format_data_for_mlm(py_data, num_answers, output_path)`: For every acronym in the data provided, tries to find num_answers possible definitions. Keyword "tries," because it will skip acronyms with less than the set number of definitions. py_data is a dict with the fields "text", "abbr", and "definition".
