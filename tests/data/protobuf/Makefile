# Default target
all: generate-desc generate-pb

# Generates .desc descriptor sets for tests
generate-desc:
	protoc --proto_path=. \
	       --include_imports \
	       --descriptor_set_out=test/v1/test.desc \
	       test/v1/test.proto

	protoc --proto_path=. \
	       --include_imports \
	       --descriptor_set_out=test_protobuf/v1/test_protobuf.desc \
	       test_protobuf/v1/test_protobuf.proto

	protoc --proto_path=. \
	       --include_imports \
	       --descriptor_set_out=test_protobuf3/v1/test_protobuf3.desc \
	       test_protobuf3/v1/test_protobuf3.proto

# Generates serialized binary .pb test files from .txt input
generate-pb:
	protoc --proto_path=. \
	       --encode=test_protobuf.v1.Person \
	       test_protobuf/v1/test_protobuf.proto \
	       < test_protobuf/v1/input/person_someone.txt > test_protobuf/v1/input/person_someone.pb

	protoc --proto_path=. \
	       --encode=test_protobuf3.v1.Person \
	       test_protobuf3/v1/test_protobuf3.proto \
	       < test_protobuf3/v1/input/person_someone.txt > test_protobuf3/v1/input/person_someone.pb

	protoc --proto_path=. \
	       --encode=test_protobuf3.v1.Person \
	       test_protobuf3/v1/test_protobuf3.proto \
	       < test_protobuf3/v1/input/person_with_job.txt > test_protobuf3/v1/input/person_with_job.pb

# Clean generated files
clean:
	rm -f test/**/*.desc test_protobuf/**/*.desc test_protobuf3/**/*.desc
	rm -f test_protobuf/input/*.pb
