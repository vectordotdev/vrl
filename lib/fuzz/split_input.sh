#!/bin/bash

# Splits the "inputs.txt" file into multiple inputs so AFL can use them

input_file="inputs.txt"
output_dir="in"
output_file_prefix="test"

rm -rf "$output_dir"
mkdir -p "$output_dir"

line_number=1
while IFS= read -r line; do
    output_file="${output_dir}/${output_file_prefix}${line_number}.txt"
    echo "$line" > "$output_file"
    line_number=$((line_number + 1))
done < "$input_file"
