@echo off
@echo Cleaning
echo bogus > example\bogus.class
del /Q example\*.class
echo bogus > src\com\artifex\R2H-PDF\fitz\bogus.class
del /Q src\com\artifex\R2H-PDF\fitz\*.class

@echo Building Viewer
javac -classpath src -sourcepath src -source 1.7 -target 1.7 example/Viewer.java example/ViewerCore.java example/PageCanvas.java example/Worker.java

@echo Building JNI classes
javac -sourcepath src -source 1.7 -target 1.7 src/com/artifex/R2H-PDF/fitz/*.java

@echo Importing DLL (%1) (built using VS solution)
@copy ..\win32\%1\javaviewerlib.dll R2H-PDF_java.dll /y

@echo Packaging into jar (incomplete as missing manifest)
jar cf R2H-PDF-java-viewer.jar R2H-PDF_java.dll src\com\artifex\R2H-PDF\fitz\*.class example\*.class
