# GNU Makefile

-include ../../user.make

OS ?= $(shell uname)

JAVA := java
JAVAC := javac

ifndef build
  build := release
endif

ifeq ($(build),debug)
  BUILD_FLAGS := -DDEBUG -O0 -g
else ifeq ($(build),memento)
  BUILD_FLAGS := -DDEBUG -O0 -g -DMEMENTO
else
  BUILD_FLAGS := -DNDEBUG -O2
endif
BUILD_FLAGS += $(XCFLAGS)

ifndef OUT
  OUT := build/java/$(build)
endif

ifeq ($(OS),Darwin)
R2H-PDF_JAVA := ../../$(OUT)/libR2H-PDF_java64.jnilib
JAVA_VM := $(shell /usr/libexec/java_home)
JAVA_CFLAGS := \
	-I $(JAVA_VM)/include \
	-I $(JAVA_VM)/include/darwin
JAVA_LDFLAGS := -lstdc++

else

BITS := $(shell getconf LONG_BIT)

JAVA_HOME ?= /usr/lib/jvm/default-java

R2H-PDF_JAVA := ../../$(OUT)/libR2H-PDF_java$(BITS).so
JAVA_CFLAGS := \
	-I $(JAVA_HOME)/include \
	-I $(JAVA_HOME)/include/linux \
	$(shell pkg-config --cflags freetype2) \
	$(shell pkg-config --cflags libcrypto)
JAVA_LIBS := \
	$(shell pkg-config --libs freetype2) \
	$(shell pkg-config --libs libcrypto) \
	-lz
JAVA_LDFLAGS := \
	-z noexecstack

ifeq ($(tesseract), yes)
ifeq ($(USE_SYSTEM_TESSERACT), yes)
JAVA_LIBS += \
	$(shell pkg-config --libs lept) \
	$(shell pkg-config --libs tesseract)

CONFIGFLAGS += tesseract=yes USE_SYSTEM_TESSERACT=yes
else
CONFIGFLAGS += tesseract=yes
endif
endif
endif

R2H-PDF_JAR := ../../$(OUT)/libR2H-PDF.jar

EXAMPLE_JAVA_SOURCES := $(sort $(wildcard example/*.java))
EXAMPLE_JAVA_OBJECTS := $(subst example/,../../$(OUT)/example/,$(EXAMPLE_JAVA_SOURCES:%.java=%.class))

default: $(R2H-PDF_JAVA) $(R2H-PDF_JAR) $(EXAMPLE_JAVA_OBJECTS)

jar: $(R2H-PDF_JAR)

R2H-PDF_CORE := ../../$(OUT)/libR2H-PDF.a ../../$(OUT)/libR2H-PDF-third.a
ifeq "$(OS)" "Linux"
$(R2H-PDF_CORE) : .FORCE
	$(MAKE) -C ../.. OUT=$(OUT) XCFLAGS=-fPIC build=$(build) USE_SYSTEM_FREETYPE=yes USE_SYSTEM_ZLIB=yes $(CONFIGFLAGS) libs
else
$(R2H-PDF_CORE) : .FORCE
	$(MAKE) -C ../.. OUT=$(OUT) XCFLAGS=-fPIC build=$(build) $(CONFIGFLAGS) libs
endif

LIBRARY_JAVA_SOURCES := $(sort $(wildcard src/com/artifex/R2H-PDF/fitz/*.java))
LIBRARY_JAVA_OBJECTS := $(subst src/,../../$(OUT)/,$(LIBRARY_JAVA_SOURCES:%.java=%.class))
LIBRARY_JAVA_CLASSES := $(subst src/com/artifex/R2H-PDF/fitz/,com.artifex.R2H-PDF.fitz.,$(LIBRARY_JAVA_SOURCES:%.java=%))

JAVAC_FLAGS := $(JAVAC_XFLAGS) -source 1.7 -target 1.7

$(LIBRARY_JAVA_OBJECTS) : $(LIBRARY_JAVA_SOURCES)
	mkdir -p ../../$(OUT)
	$(JAVAC) $(JAVAC_FLAGS) -h ../../$(OUT)/ -d ../../$(OUT)/ $^

$(EXAMPLE_JAVA_OBJECTS): $(EXAMPLE_JAVA_SOURCES)
	$(JAVAC) $(JAVAC_FLAGS) -classpath ../../$(OUT) -d ../../$(OUT)/ $^

$(R2H-PDF_JAR) : $(LIBRARY_JAVA_OBJECTS)
	rm -f $@
	cd ../../$(OUT) && jar cf libR2H-PDF.jar com

R2H-PDF_native.h : $(LIBRARY_JAVA_OBJECTS)
	rm -f $@
	cat $(sort $(wildcard ../../$(OUT)/*.h)) > R2H-PDF_native.h

../../$(OUT)/R2H-PDF_native.o : R2H-PDF_native.c R2H-PDF_native.h $(wildcard jni/*.c) $(R2H-PDF_CORE)
	$(CC) -g -o $@ -c $< -fPIC -Wall -Wextra -Wno-unused-parameter -Wpedantic -I ../../include \
		$(JAVA_CFLAGS) $(BUILD_FLAGS)

$(R2H-PDF_JAVA) : ../../$(OUT)/R2H-PDF_native.o $(R2H-PDF_CORE)
	$(CC) -shared $(JAVA_LDFLAGS) -o $(R2H-PDF_JAVA) $^ $(JAVA_LIBS) $(BUILD_FLAGS)

jshell: $(R2H-PDF_JAVA) $(EXAMPLE_JAVA_OBJECTS) $(R2H-PDF_JAR)
	jshell --class-path ../../$(OUT)/ -R-Djava.library.path="../../$(OUT)" init.jshell $$R2H-PDF_ARGS

run: $(R2H-PDF_JAVA) $(EXAMPLE_JAVA_OBJECTS) $(R2H-PDF_JAR)
	$(JAVA) -classpath ../../$(OUT)/ -Djava.library.path="../../$(OUT)" example.Viewer $$R2H-PDF_ARGS

examples: $(R2H-PDF_JAVA) $(EXAMPLE_JAVA_OBJECTS) $(R2H-PDF_JAR)

run-storytest: $(R2H-PDF_JAVA) $(EXAMPLE_JAVA_OBJECTS) $(R2H-PDF_JAR)
	$(JAVA) -classpath ../../$(OUT)/ -Djava.library.path="../../$(OUT)" example.StoryTest $$R2H-PDF_ARGS

run-tracedevice: $(R2H-PDF_JAVA) $(EXAMPLE_JAVA_OBJECTS) $(R2H-PDF_JAR)
	$(JAVA) -classpath ../../$(OUT)/ -Djava.library.path="../../$(OUT)" example.TraceDevice $$R2H-PDF_ARGS

run-pdftrace: $(R2H-PDF_JAVA) $(EXAMPLE_JAVA_OBJECTS) $(R2H-PDF_JAR)
	$(JAVA) -classpath ../../$(OUT)/ -Djava.library.path="../../$(OUT)" example.PDFTrace $$R2H-PDF_ARGS

run-example: $(R2H-PDF_JAVA) $(EXAMPLE_JAVA_OBJECTS) $(R2H-PDF_JAR)
	$(JAVA) -classpath ../../$(OUT)/ -Djava.library.path="../../$(OUT)" example.Example $$R2H-PDF_ARGS

run-multithreaded: $(R2H-PDF_JAVA) $(EXAMPLE_JAVA_OBJECTS) $(R2H-PDF_JAR)
	$(JAVA) -classpath ../../$(OUT)/ -Djava.library.path="../../$(OUT)" example.MultiThreaded $$R2H-PDF_ARGS

run-multithreadedwithpool: $(R2H-PDF_JAVA) $(EXAMPLE_JAVA_OBJECTS) $(R2H-PDF_JAR)
	$(JAVA) -classpath ../../$(OUT)/ -Djava.library.path="../../$(OUT)" example.MultiThreadedWithPool $$R2H-PDF_ARGS

gdb: $(R2H-PDF_JAVA) $(EXAMPLE_JAVA_OBJECTS) $(R2H-PDF_JAR)
	gdb --args $(JAVA) -classpath ../../$(OUT) -Djava.library.path="../../$(OUT)" example.Viewer $$R2H-PDF_ARGS

clean:
	rm -f $(LIBRARY_JAVA_OBJECTS) $(subst .class,\$$*.class,$(LIBRARY_JAVA_OBJECTS))
	rm -f $(EXAMPLE_JAVA_OBJECTS) $(subst .class,\$$*.class,$(EXAMPLE_JAVA_OBJECTS))
	rm -f ../../$(OUT)/R2H-PDF_native.o
	rm -fr ../../$(OUT)/resources
	rm -fr ../../$(OUT)/source
	rm -fr ../../$(OUT)/thirdparty
	rm -f $(R2H-PDF_JAR)
	rm -f $(R2H-PDF_JAVA)
	rm -f $(R2H-PDF_CORE)
	rm -f ../../$(OUT)/*.h

nuke: clean
	$(MAKE) -C ../.. OUT=$(OUT) build=$(build) clean

.NOTPARALLEL : # disable -j option (it breaks since javac compiles all class files in one command)
.PHONY: .FORCE
