@echo off
echo Fetching BnD-Ware Repository...
git clone https://github.com/VegeSushi/BnD-Ware temp_bnd

echo Creating core directories...
mkdir core\minigames
mkdir core\assets

echo Copying files...
xcopy /E /I temp_bnd\filesystem\minigames core\minigames
xcopy /E /I temp_bnd\assets core\assets

echo Cleaning up...
rmdir /S /Q temp_bnd

echo Compressing core package...
powershell Compress-Archive -Path core\* -DestinationPath core.zip
rename core.zip core.bnd

echo Fetch and core.bnd generation complete!
pause